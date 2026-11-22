//! MicroDVD (`.sub` text): `{start}{end}text` with frame numbers, `|` between lines, and
//! control codes in braces (`{y:i}` italic, `{c:$0000FF}` a color in blue-green-red).
//!
//! Frame numbers need the video's frame rate. The app reads it only from the file's own first
//! line (`{1}{1}23.976`); without one, libVLC, which knows the video's, draws the file.

use std::time::Duration;

use super::markup::Builder;
use super::{Cue, Emphasis};

/// The frame numbers a header line may carry.
const HEADER_FRAMES: [u64; 2] = [0, 1];

/// The cues of a MicroDVD file, or `None` when it doesn't say its frame rate (or isn't
/// MicroDVD: SubViewer `.sub` files are timed differently).
pub(super) fn parse(text: &str) -> Option<Vec<Cue>> {
    let mut lines = text.lines().filter_map(frames);
    let (start, end, header) = lines.next()?;
    let fps: f64 = header.trim().parse().ok().filter(|fps: &f64| *fps > 0.)?;
    if !HEADER_FRAMES.contains(&start) || !HEADER_FRAMES.contains(&end) {
        return None;
    }
    let time = |frame: u64| Duration::from_secs_f64(frame as f64 / fps);
    let cues = lines.map(|(start, end, text)| Cue {
        start: time(start),
        end: time(end),
        spans: spans(text),
        placement: super::Placement::Bottom,
    });
    Some(cues.collect())
}

/// A line's start and end frames and its text.
fn frames(line: &str) -> Option<(u64, u64, &str)> {
    let (start, rest) = line.trim().strip_prefix('{')?.split_once('}')?;
    let (end, text) = rest.strip_prefix('{')?.split_once('}')?;
    Some((start.parse().ok()?, end.parse().ok()?, text))
}

/// A cue's text: `|` breaks the line; `{y:…}` codes emphasize, `{c:$BBGGRR}` colors, and the
/// other codes (fonts, sizes, positions) are dropped.
fn spans(text: &str) -> Vec<super::Span> {
    let mut builder = Builder::default();
    let mut rest = text;
    while let Some(open) = rest.find(['{', '|']) {
        builder.text(&rest[..open]);
        rest = &rest[open..];
        if let Some(after) = rest.strip_prefix('|') {
            builder.text("\n");
            rest = after;
            continue;
        }
        let Some(close) = rest.find('}') else { break };
        control(&mut builder, &rest[1..close]);
        rest = &rest[close + 1..];
    }
    builder.text(rest);
    builder.finish().0
}

/// Applies one control code.
fn control(builder: &mut Builder, code: &str) {
    let Some((kind, value)) = code.split_once(':') else { return };
    match kind {
        "y" | "Y" => {
            let value = value.to_ascii_lowercase();
            let emphases =
                [('i', Emphasis::ITALIC), ('b', Emphasis::BOLD), ('u', Emphasis::UNDERLINE)];
            for (letter, emphasis) in emphases {
                if value.contains(letter) {
                    builder.emphasize(emphasis, true);
                }
            }
        }
        "c" | "C" => {
            let bgr = value.trim_start_matches('$');
            if let Ok(bgr) = u32::from_str_radix(bgr, 16) {
                let rgb = ((bgr & 0xFF) << 16) | (bgr & 0xFF00) | ((bgr >> 16) & 0xFF);
                builder.color(rgb);
            }
        }
        _ => {}
    }
}
