//! Video's HDR (S7.5): how HDR video is shown. The player's settings panel holds the same row
//! (its Video tab's Defaults) and changes it for the session only (D24).

use super::super::def::{Control, SettingDef};
use super::super::value::{Choice, SettingChoice, choice};
use crate::engine::hdr::HdrMode;
use crate::settings::registry::Category;

const HDR: (Category, &str) = (Category::Video, "HDR");

impl SettingChoice for HdrMode {
    const VALUES: &'static [Self] = &[Self::Auto, Self::Off, Self::Hdr10, Self::Hlg];
    const CHOICES: &'static [Choice] = &[
        Choice::new("auto", "Auto"),
        Choice::new("off", "Off"),
        Choice::new("hdr10", "HDR10"),
        Choice::new("hlg", "HLG"),
    ];
}

/// Says when the player's panel changed the HDR mode for this session, so this page isn't what
/// shows (D24).
fn hdr_overridden(cx: &gpui::App) -> Option<String> {
    crate::effects::is_changed(cx, "video.hdr").then(|| {
        "The player's settings panel changed this for the session; its Save as Default writes it \
         here."
            .to_owned()
    })
}

/// The HDR row.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[SettingDef::new(
    "video.hdr",
    HDR,
    "HDR",
    Control::Dropdown(HdrMode::CHOICES),
    choice!(prefs.video.hdr),
)
.info(
    "How HDR video shows. VLC turns it into SDR for every display (tone mapping), decoding it \
     on the CPU. Auto does so for video whose metadata says it is HDR10 or HLG; HDR10 and HLG \
     read every video as that, for files tagged wrongly; Off shows HDR as decoded, washed out, \
     but keeps hardware decoding. A change reopens the video where it is.",
)
.hint(hdr_overridden)
.keywords(&["hdr", "tone mapping", "hdr10", "hlg", "dolby vision", "pq", "washed out"])];
