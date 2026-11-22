//! Playback ("Playback Preferences"): starting, resuming, the queue's order and when media counts
//! as watched.

use super::super::def::{ApplyMode, Condition, Control, SettingDef};
use super::super::rules::trim;
use super::super::value::{Choice, SettingChoice, choice, number, toggle};
use crate::settings::registry::Category;
use crate::settings::{QueueSiblings, ResumePolicy, WhenFinished};

const STARTUP: (Category, &str) = (Category::Playback, "Startup");
const ORDER: (Category, &str) = (Category::Playback, "Order");
const HISTORY: (Category, &str) = (Category::Playback, "History");

impl SettingChoice for ResumePolicy {
    const VALUES: &'static [Self] = &[Self::Always, Self::Ask, Self::Never];
    const CHOICES: &'static [Choice] = &[
        Choice::new("always", "Always"),
        Choice::new("ask", "Ask"),
        Choice::new("never", "Never"),
    ];
}

impl SettingChoice for QueueSiblings {
    const VALUES: &'static [Self] = &[Self::Never, Self::Episodes, Self::Always];
    const CHOICES: &'static [Choice] = &[
        Choice::new("never", "Never"),
        Choice::new("episodes", "For TV episodes"),
        Choice::new("always", "Always"),
    ];
}

impl SettingChoice for WhenFinished {
    const VALUES: &'static [Self] = &WhenFinished::ALL;
    const CHOICES: &'static [Choice] =
        &[Choice::new("stop", "Stop"), Choice::new("quit", "Quit VLC")];
}

/// A speed as the slider labels it: "1.25".
fn speed(value: f64) -> String {
    trim(value)
}

/// The Playback page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "player.restore_session",
        STARTUP,
        "Continue where you left off",
        Control::Toggle,
        toggle!(prefs.player.restore_session),
    )
    .info("Brings back the last queue when VLC starts, paused where you stopped.")
    .keywords(&["session", "restore", "last queue", "startup"]),
    SettingDef::new(
        "player.resume",
        STARTUP,
        "Continue playback",
        Control::Dropdown(ResumePolicy::CHOICES),
        choice!(prefs.player.resume),
    )
    .info(
        "What happens when you open a file you stopped partway through: continue from there, \
         start over and offer to continue, or always start over. Needs \"Remember the playback \
         position\" in Privacy.",
    )
    .depends_on("privacy.remember_playback_position", Condition::IsOn)
    .keywords(&["resume", "playback position", "start over"]),
    SettingDef::new(
        "player.default_speed",
        STARTUP,
        "Default playback speed",
        Control::Slider { marks: Some(0.25), format: speed },
        number!(f32: prefs.player.default_speed),
    )
    .info("The speed VLC starts at. A speed you pick while watching lasts until VLC quits.")
    .range(0.25, 2., 0.05)
    .applies(ApplyMode::Restart)
    .keywords(&["rate", "faster", "slower"]),
    SettingDef::new(
        "player.pause_when_another_app_uses_audio",
        STARTUP,
        "Pause when another app uses audio",
        Control::Toggle,
        toggle!(prefs.player.pause_when_another_app_uses_audio),
    )
    .info("Pauses while a call or another app takes over the sound, and plays on after it.")
    .keywords(&["interruption", "call", "ducking"]),
    SettingDef::new(
        "player.queue_siblings",
        ORDER,
        "Queue the rest of the folder",
        Control::Dropdown(QueueSiblings::CHOICES),
        choice!(prefs.player.queue_siblings),
    )
    .info(
        "Whether opening one file also queues the other media in its folder, in order, so the \
         next episode plays after it.",
    )
    .keywords(&["playlist", "next episode", "binge", "autoplay"]),
    SettingDef::new(
        "player.stop_after_each_item",
        ORDER,
        "Stop after each item",
        Control::Toggle,
        toggle!(prefs.player.stop_after_each_item),
    )
    .info("Waits at the end of every item in the queue. Play goes on to the next one.")
    .keywords(&["queue", "pause between"]),
    SettingDef::new(
        "player.when_finished",
        ORDER,
        "When playback finishes",
        Control::Dropdown(WhenFinished::CHOICES),
        choice!(prefs.player.when_finished),
    )
    .info("What VLC does when the last item in the queue ends.")
    .keywords(&["end of queue", "quit", "close"]),
    SettingDef::new(
        "player.watched_percent",
        HISTORY,
        "Watched after",
        Control::Number { unit: "%" },
        number!(u32: prefs.player.watched_percent),
    )
    .info(
        "Media counts as watched once you are this far in, or near its end (whichever comes \
         first), and leaves Continue Watching.",
    )
    .range(1., 100., 1.)
    .keywords(&["finished", "continue watching", "completed"])
    .advanced(),
    SettingDef::new(
        "player.watched_remaining_seconds",
        HISTORY,
        "Or with this much left",
        Control::Number { unit: "s" },
        number!(u32: prefs.player.watched_remaining_seconds),
    )
    .info("Media also counts as watched this close to its end, so end credits don't matter.")
    .range(0., 3600., 1.)
    .keywords(&["finished", "credits", "continue watching"])
    .advanced(),
];
