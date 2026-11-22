//! Text subtitle cues the app draws itself (S6.2): a subtitle file read into timed lines of
//! styled text, looked up by the time the video is at.
//!
//! libVLC 3 draws subtitles into the decoded picture, with a style it reads only when its
//! instance starts, so the app draws the text files it can read itself: their style changes
//! apply at once, they can sit in the black bars and they move above the control bar. Picture
//! subtitles, formats read here as `None`, and styled ASS/SSA while Embedded Sub Format is on
//! stay with libVLC.
//!
//! - [`time`]: timestamps as the formats write them.
//! - [`markup`]: the HTML-like tags of SRT and WebVTT, and the override tags SRT borrows from ASS.
//! - [`blocks`]: SRT and WebVTT, a timing line and its text in blocks.
//! - [`ass`]: ASS/SSA dialogue lines.
//! - [`microdvd`]: MicroDVD's frame-numbered lines.

mod ass;
mod blocks;
mod markup;
mod microdvd;
mod time;

use std::path::Path;
use std::time::Duration;

/// One timed piece of a subtitle file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cue {
    /// When it appears, in media time.
    pub start: Duration,
    /// When it goes.
    pub end: Duration,
    /// Its text: runs of one style each, a `\n` inside a run breaking the line.
    pub spans: Vec<Span>,
    /// Where on the picture it sits.
    pub placement: Placement,
}

impl Cue {
    /// The cue's text without its styles.
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

/// A run of text in one style.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Span {
    /// The text; `\n` breaks the line.
    pub text: String,
    /// How it is emphasized.
    pub emphasis: Emphasis,
    /// The color the file gives it, `0xRRGGBB`, drawn only while Embedded Sub Format is on.
    pub color: Option<u32>,
}

/// Italic, bold and underline, any of them together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Emphasis(u8);

impl Emphasis {
    /// Plain text.
    pub const NONE: Self = Self(0);
    /// Slanted.
    pub const ITALIC: Self = Self(1);
    /// Heavier.
    pub const BOLD: Self = Self(2);
    /// Underlined.
    pub const UNDERLINE: Self = Self(4);

    /// Whether every emphasis in `other` is here.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// This emphasis with `other` added (`on`) or taken away.
    #[must_use]
    pub fn with(self, other: Self, on: bool) -> Self {
        if on { Self(self.0 | other.0) } else { Self(self.0 & !other.0) }
    }
}

/// Where a cue sits on the picture: ASS's `\an`, WebVTT's `line`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Placement {
    /// Along the bottom, where subtitles go.
    #[default]
    Bottom,
    /// In the middle.
    Middle,
    /// Along the top, often signs or a second speaker.
    Top,
}

/// The cues of one subtitle file, sorted by when they start.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CueTrack {
    cues: Vec<Cue>,
    /// The longest a cue lasts, which bounds how far back a lookup searches.
    longest: Duration,
    /// The file styles its own text (ASS/SSA).
    styled: bool,
}

impl CueTrack {
    /// A track of `cues` in any order. Cues that end before they start or hold no text are
    /// dropped; ones that overlap stay, and show together.
    pub fn new(mut cues: Vec<Cue>, styled: bool) -> Self {
        cues.retain(|cue| cue.end > cue.start && !cue.text().trim().is_empty());
        cues.sort_by_key(|cue| cue.start);
        let longest =
            cues.iter().map(|cue| cue.end.saturating_sub(cue.start)).max().unwrap_or_default();
        Self { cues, longest, styled }
    }

    /// The cues showing at `time`, earliest first.
    pub fn at(&self, time: Duration) -> impl Iterator<Item = &Cue> {
        let end = self.cues.partition_point(|cue| cue.start <= time);
        let from = self.cues[..end].partition_point(|cue| cue.start + self.longest <= time);
        self.cues[from..end].iter().filter(move |cue| cue.end > time)
    }

    /// Whether the file styles its own text, so Embedded Sub Format on leaves it to libVLC.
    pub fn is_styled(&self) -> bool {
        self.styled
    }

    /// How many cues the file holds.
    pub fn len(&self) -> usize {
        self.cues.len()
    }

    /// Whether the file holds no cues.
    pub fn is_empty(&self) -> bool {
        self.cues.is_empty()
    }
}

/// Reads `text`, the contents of the subtitle file at `path`, into cues, by the file's
/// extension. `None` for a format libVLC draws instead (pictures, TTML, SAMI, the rarer text
/// formats, MicroDVD without its frame rate) or a file with no cue in it.
pub fn parse(path: &Path, text: &str) -> Option<CueTrack> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    // A byte order mark can start the file, or sit in the middle of a joined one.
    let text = text.replace('\u{FEFF}', "");
    let track = match extension.as_str() {
        "srt" => CueTrack::new(blocks::subrip(&text), false),
        "vtt" => CueTrack::new(blocks::webvtt(&text), false),
        "ass" | "ssa" => CueTrack::new(ass::parse(&text), true),
        "sub" => CueTrack::new(microdvd::parse(&text)?, false),
        _ => return None,
    };
    if track.is_empty() {
        tracing::info!("subtitles: no cue read in {}", crate::logging::path(path));
        return None;
    }
    Some(track)
}

#[cfg(test)]
mod tests;
