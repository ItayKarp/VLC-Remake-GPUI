//! Playback ("Playback Preferences"), second half: the queue's order, seeking, and when media
//! counts as watched.
//!
//! Repeat and Shuffle are the queue's own state (S3.3.4): their rows bind to the
//! [`LiveState`](crate::settings::LiveState) mirror, which the main window keeps in step with the
//! queue both ways.

use super::super::def::{Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, toggle};
use crate::queue::Repeat;
use crate::settings::registry::Category;
use crate::settings::{QueueSiblings, WhenFinished};

const ORDER: (Category, &str) = (Category::Playback, "Order");
const SEEKING: (Category, &str) = (Category::Playback, "Seeking");
const HISTORY: (Category, &str) = (Category::Playback, "History");

impl SettingChoice for Repeat {
    const VALUES: &'static [Self] = &Repeat::ALL;
    const CHOICES: &'static [Choice] =
        &[Choice::new("off", "Off"), Choice::new("all", "All"), Choice::new("one", "One")];
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

/// "Shuffle": the queue's own.
const SHUFFLE: Binding = Binding {
    get: |s| Value::Bool(s.live.shuffle),
    set: |s, value| {
        if let Value::Bool(on) = value {
            s.live.shuffle = on;
        }
    },
};

/// The Playback page's Order, Seeking and History rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "queue.repeat",
        ORDER,
        "Repeat",
        Control::Dropdown(Repeat::CHOICES),
        choice!(live.repeat),
    )
    .info(
        "Off plays the queue once; All starts it over after the last item; One plays the \
         current item again. The same as the queue's Repeat button.",
    )
    .keywords(&["loop", "repeat one", "repeat all", "playlist"]),
    SettingDef::new("queue.shuffle", ORDER, "Shuffle", Control::Toggle, SHUFFLE)
        .info("Plays the queue in a random order. The same as the queue's Shuffle button.")
        .keywords(&["random", "playlist", "mix"]),
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
        "player.seek_precisely",
        SEEKING,
        "Seek precisely",
        Control::Toggle,
        toggle!(prefs.player.seek_precisely),
    )
    .info(
        "Jumps land on the exact moment asked for. Off, they land on the nearest keyframe \
         instead, which is faster on large or remote files.",
    )
    .keywords(&["fast seek", "keyframe", "accurate", "frame"]),
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
