//! The registry's rows, one file per category, and the checks they share.
//!
//! Categories whose settings don't exist yet (Network, Performance, Recording, Extensions)
//! have no file: their stories add one along with the settings.

mod audio;
mod audio_effects;
mod controls;
mod diagnostics;
mod general;
mod interface;
mod library;
mod play_order;
mod playback;
mod privacy;
mod subtitle_style;
mod subtitles;
mod video;
mod window;

use super::def::SettingDef;
use super::value::Value;
use crate::engine::language::Language;

/// Every category's rows, in sidebar order (General, Interface and Playback in two parts each,
/// Audio in three, Subtitles in two).
pub(super) static PAGES: [&[SettingDef]; 15] = [
    general::ENTRIES,
    diagnostics::ENTRIES,
    window::ENTRIES,
    interface::ENTRIES,
    privacy::ENTRIES,
    controls::ENTRIES,
    playback::ENTRIES,
    play_order::ENTRIES,
    video::ENTRIES,
    audio::ENTRIES,
    audio_effects::ENTRIES,
    audio::LANGUAGE_ENTRIES,
    subtitles::ENTRIES,
    subtitle_style::ENTRIES,
    library::ENTRIES,
];

/// Checks that every entry of a language list is a language code.
fn languages(value: &Value) -> Result<(), String> {
    let Value::List(codes) = value else { return Ok(()) };
    match codes.iter().find(|code| Language::parse(code).is_none()) {
        Some(code) => Err(format!("\"{code}\" isn't a language code")),
        None => Ok(()),
    }
}
