//! SRT and WebVTT: blocks of a timing line (`00:00:01,000 --> 00:00:02,500`) and the lines of
//! text under it.
//!
//! Read leniently, as players do: numbering and cue names are ignored, a block also ends at the
//! next timing line (or a number right above one) when the blank line between is missing,
//! either decimal separator works, and a block whose timing doesn't read is skipped and counted
//! in the log. Cues are sorted afterwards ([`super::CueTrack::new`]).

use super::markup::Builder;
use super::{Cue, Placement, time};

/// What separates a cue's start and end.
const ARROW: &str = "-->";
/// WebVTT's `line` setting as a percentage of the height: above this is the top.
const TOP_PERCENT: f32 = 100. / 3.;
/// Below this (and not at the top) is the middle.
const MIDDLE_PERCENT: f32 = 200. / 3.;

/// The cues of an SRT file.
pub(super) fn subrip(text: &str) -> Vec<Cue> {
    cues(text, "SRT")
}

/// The cues of a WebVTT file. Its header, `NOTE`, `STYLE` and `REGION` blocks have no timing
/// line, so they are passed over like SRT's numbers.
pub(super) fn webvtt(text: &str) -> Vec<Cue> {
    cues(text, "WebVTT")
}

fn cues(text: &str, format: &str) -> Vec<Cue> {
    let lines: Vec<&str> = text.lines().collect();
    let mut cues = Vec::new();
    let mut skipped = 0_usize;
    let mut index = 0;
    while index < lines.len() {
        let timing = lines[index];
        index += 1;
        if !timing.contains(ARROW) {
            continue;
        }
        let first = index;
        while index < lines.len() && !ends_block(&lines, index) {
            index += 1;
        }
        match cue(timing, &lines[first..index]) {
            Some(cue) => cues.push(cue),
            None => skipped += 1,
        }
    }
    if skipped > 0 {
        tracing::info!("subtitles: skipped {skipped} {format} cues whose timing didn't read");
    }
    cues
}

/// Whether the line at `index` starts another block rather than continuing the text.
fn ends_block(lines: &[&str], index: usize) -> bool {
    let line = lines[index];
    let numbered = || {
        !line.trim().is_empty()
            && line.trim().bytes().all(|byte| byte.is_ascii_digit())
            && lines.get(index + 1).is_some_and(|next| next.contains(ARROW))
    };
    line.trim().is_empty() || line.contains(ARROW) || numbered()
}

/// The cue a timing line and its text lines make.
fn cue(timing: &str, lines: &[&str]) -> Option<Cue> {
    let (start, rest) = timing.split_once(ARROW)?;
    let mut rest = rest.split_whitespace();
    let start = time::parse(start)?;
    let end = time::parse(rest.next()?)?;
    let mut builder = Builder::placed(rest.find_map(line_setting));
    for (number, line) in lines.iter().enumerate() {
        if number > 0 {
            builder.text("\n");
        }
        builder.html(line);
    }
    let (spans, placement) = builder.finish();
    Some(Cue { start, end, spans, placement: placement.unwrap_or_default() })
}

/// Where WebVTT's `line:` setting puts a cue: a line number counts from the top when positive
/// and from the bottom when negative; a percentage is a height.
fn line_setting(setting: &str) -> Option<Placement> {
    let value = setting.strip_prefix("line:")?.split(',').next()?;
    if let Some(percent) = value.strip_suffix('%') {
        let percent: f32 = percent.parse().ok()?;
        return Some(if percent < TOP_PERCENT {
            Placement::Top
        } else if percent < MIDDLE_PERCENT {
            Placement::Middle
        } else {
            Placement::Bottom
        });
    }
    let line: f32 = value.parse().ok()?;
    Some(if line < 0. { Placement::Bottom } else { Placement::Top })
}
