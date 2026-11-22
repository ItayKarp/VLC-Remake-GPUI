//! How new media opens, beyond where it starts (Settings → Playback, S5.7): the stream quality
//! adaptive streams pick, the language disc menus show in, and how precisely seeks land.

use super::language::Language;

/// Options every media opened from now on gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MediaOptions {
    /// The most lines an adaptive stream (HLS, DASH) may pick; `None` for the best available.
    pub max_lines: Option<u32>,
    /// The language DVD and Blu-ray menus show in; `None` lets the disc choose.
    pub menu_language: Option<Language>,
    /// Whether seeks land on the exact moment (`true`) or the nearest keyframe, faster.
    pub precise_seeks: bool,
}

impl Default for MediaOptions {
    fn default() -> Self {
        Self { max_lines: None, menu_language: None, precise_seeks: true }
    }
}
