//! Playback ("Playback Preferences"), first half: how media starts and which of its tracks
//! play. The page's second half (the queue's order, seeking and when media counts as watched) is
//! in `play_order.rs`.

use super::super::def::{ApplyMode, Condition, Control, MachineChoice, SettingDef};
use super::super::rules::trim;
use super::super::value::{Choice, SettingChoice, choice, number, text, toggle};
use crate::engine::language::{self, Language};
use crate::settings::registry::Category;
use crate::settings::{ResumePolicy, VideoQuality};

const STARTUP: (Category, &str) = (Category::Playback, "Startup");
const TRACKS: (Category, &str) = (Category::Playback, "Tracks");
/// The highest track number the default track rows take.
const MAX_TRACK: f64 = 99.;
/// The menu language id that lets the disc choose.
const DISC_LANGUAGE: &str = "";

impl SettingChoice for ResumePolicy {
    const VALUES: &'static [Self] = &[Self::Always, Self::Ask, Self::Never];
    const CHOICES: &'static [Choice] = &[
        Choice::new("always", "Always"),
        Choice::new("ask", "Ask"),
        Choice::new("never", "Never"),
    ];
}

impl SettingChoice for VideoQuality {
    const VALUES: &'static [Self] =
        &[Self::Best, Self::Full, Self::High, Self::Standard, Self::Low, Self::Lowest];
    const CHOICES: &'static [Choice] = &[
        Choice::new("best", "Best available"),
        Choice::new("full", "Full HD (1080p)"),
        Choice::new("high", "HD (720p)"),
        Choice::new("standard", "Standard (576p)"),
        Choice::new("low", "Low (360p)"),
        Choice::new("lowest", "Lowest (240p)"),
    ];
}

/// A speed as the slider labels it: "1.25".
fn speed(value: f64) -> String {
    trim(value)
}

/// "Set by the disc", then every language by name; a saved code that isn't listed comes last,
/// so the row still shows what is saved.
fn menu_languages(_: &gpui::App, saved: &str) -> Vec<MachineChoice> {
    let disc = MachineChoice { id: DISC_LANGUAGE.into(), label: "Set by the disc".into() };
    let mut choices = vec![disc];
    choices.extend(
        language::known()
            .iter()
            .map(|known| MachineChoice { id: known.code().into(), label: known.name() }),
    );
    if !choices.iter().any(|choice| choice.id == saved) {
        let label = Language::parse(saved).map_or_else(|| saved.to_owned(), |l| l.name());
        choices.push(MachineChoice { id: saved.into(), label });
    }
    choices
}

/// The Playback page's Startup and Tracks rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "player.resume",
        STARTUP,
        "Continue where you left off",
        Control::Dropdown(ResumePolicy::CHOICES),
        choice!(prefs.player.resume),
    )
    .info(
        "What happens when you open a file you stopped partway through: continue from there, \
         start over and offer to continue, or always start over. Needs \"Remember the playback \
         position\" in Privacy.",
    )
    .depends_on("privacy.remember_playback_position", Condition::IsOn)
    .keywords(&["resume", "playback position", "start over", "continue playback"]),
    SettingDef::new(
        "player.start_playing",
        STARTUP,
        "Start playing as soon as a file opens",
        Control::Toggle,
        toggle!(prefs.player.start_playing),
    )
    .info(
        "Plays what you open right away. Off, it waits paused at its start until you press \
         Play; the next items in the queue still play on their own.",
    )
    .keywords(&["autoplay", "start paused", "auto start"]),
    SettingDef::new(
        "player.default_speed",
        STARTUP,
        "Default playback speed",
        Control::Slider { marks: Some(0.25), format: speed },
        number!(f32: prefs.player.default_speed),
    )
    .info("The speed VLC starts at. A speed you pick while watching lasts until VLC quits.")
    .range(0.25, 2., 0.25)
    .applies(ApplyMode::Restart)
    .keywords(&["rate", "faster", "slower"]),
    SettingDef::new(
        "player.pause_when_another_app_uses_audio",
        STARTUP,
        "Pause when another app uses audio",
        Control::Toggle,
        toggle!(prefs.player.pause_when_another_app_uses_audio),
    )
    .info(
        "Pauses while a call or another app takes over the sound, and plays on after it. \
         Windows and Linux say when this happens; macOS doesn't, so there it never pauses.",
    )
    .keywords(&["interruption", "call", "ducking", "audio focus"]),
    SettingDef::new(
        "player.restore_session",
        STARTUP,
        "Bring back the last queue",
        Control::Toggle,
        toggle!(prefs.player.restore_session),
    )
    .info("Brings back the last queue when VLC starts, paused where you stopped.")
    .keywords(&["session", "restore", "last queue", "startup"]),
    SettingDef::new(
        "player.video_quality",
        TRACKS,
        "Preferred video quality",
        Control::Dropdown(VideoQuality::CHOICES),
        choice!(prefs.player.video_quality),
    )
    .info(
        "The resolution streams that offer several (HLS, DASH) play at. A lower one saves data \
         on a slow or metered connection.",
    )
    .applies(ApplyMode::NextMedia)
    .keywords(&["resolution", "adaptive", "bandwidth", "1080p", "720p", "stream"]),
    SettingDef::new(
        "player.menu_language",
        TRACKS,
        "Preferred menu language",
        Control::Options(menu_languages),
        text!(prefs.player.menu_language),
    )
    .info("The language DVD and Blu-ray menus show in, when the disc has it.")
    .applies(ApplyMode::NextMedia)
    .keywords(&["dvd", "blu-ray", "bluray", "disc", "menu"]),
    SettingDef::new(
        "player.default_audio_track",
        TRACKS,
        "Default audio track",
        Control::Number { unit: "" },
        number!(u32: prefs.player.default_audio_track),
    )
    .info(
        "The audio track to play, counted from 1, when none is in a preferred language (Audio \
         page). 0 keeps the media's own choice, as does a number past its last track.",
    )
    .range(0., MAX_TRACK, 1.)
    .applies(ApplyMode::NextMedia)
    .keywords(&["audio track", "dub", "commentary"]),
    SettingDef::new(
        "player.default_subtitle_track",
        TRACKS,
        "Default subtitle track",
        Control::Number { unit: "" },
        number!(u32: prefs.player.default_subtitle_track),
    )
    .info(
        "The subtitle track to show, counted from 1, when none is in a preferred language. 0 \
         keeps the media's own choice, as does a number past its last track.",
    )
    .range(0., MAX_TRACK, 1.)
    .depends_on("subtitles.enabled", Condition::IsOn)
    .applies(ApplyMode::NextMedia)
    .keywords(&["subtitle track", "captions", "subs"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui::test]
    fn menu_languages_start_with_the_discs_own_and_keep_the_saved_one(
        cx: &mut gpui::TestAppContext,
    ) {
        let ids = |saved: &str| -> Vec<String> {
            cx.update(|cx| menu_languages(cx, saved).into_iter().map(|c| c.id).collect())
        };
        let listed = ids("en");
        assert_eq!(listed.first().map(String::as_str), Some(DISC_LANGUAGE));
        assert!(listed.iter().any(|id| id == "en"));
        assert_eq!(ids("tlh").last().map(String::as_str), Some("tlh"));
    }
}
