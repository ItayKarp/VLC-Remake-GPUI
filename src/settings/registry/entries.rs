//! The registry's rows, one file per category, and the checks they share.
//!
//! Extensions has no file: its add-ons list waits for the extension system (S8.14, D19).

mod audio;
mod audio_effects;
mod controls;
mod diagnostics;
mod general;
mod interface;
mod library;
mod network;
mod performance;
mod play_order;
mod playback;
mod privacy;
mod recording;
mod subtitle_style;
pub(super) mod subtitles;
mod video;
mod video_colors;
mod video_framing;
mod window;

use super::def::SettingDef;
use super::value::Value;
use crate::engine::language::Language;

/// Every category's rows, in sidebar order (General, Interface and Playback in two parts each,
/// Video and Audio in three, Subtitles in two).
pub(super) static PAGES: [&[SettingDef]; 20] = [
    general::ENTRIES,
    diagnostics::ENTRIES,
    window::ENTRIES,
    interface::ENTRIES,
    privacy::ENTRIES,
    controls::ENTRIES,
    playback::ENTRIES,
    play_order::ENTRIES,
    video::ENTRIES,
    video_framing::ENTRIES,
    video_colors::ENTRIES,
    audio::ENTRIES,
    audio_effects::ENTRIES,
    audio::LANGUAGE_ENTRIES,
    subtitles::ENTRIES,
    subtitle_style::ENTRIES,
    library::ENTRIES,
    network::ENTRIES,
    performance::ENTRIES,
    recording::ENTRIES,
];

/// Checks that every entry of a language list is a language code.
fn languages(value: &Value) -> Result<(), String> {
    let Value::List(codes) = value else { return Ok(()) };
    match codes.iter().find(|code| Language::parse(code).is_none()) {
        Some(code) => Err(format!("\"{code}\" isn't a language code")),
        None => Ok(()),
    }
}
