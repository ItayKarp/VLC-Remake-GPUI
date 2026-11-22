//! The registry's rows, one file per category, and the checks they share.
//!
//! Categories whose settings don't exist yet (Video, Network, Performance, Recording, Extensions)
//! have no file: their stories add one along with the settings.

mod audio;
mod controls;
mod general;
mod interface;
mod library;
mod playback;
mod privacy;
mod subtitles;

use super::def::SettingDef;
use super::value::Value;
use crate::engine::language::Language;

/// Every category's rows, in sidebar order.
pub(super) static PAGES: [&[SettingDef]; 8] = [
    general::ENTRIES,
    interface::ENTRIES,
    privacy::ENTRIES,
    controls::ENTRIES,
    playback::ENTRIES,
    audio::ENTRIES,
    subtitles::ENTRIES,
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
