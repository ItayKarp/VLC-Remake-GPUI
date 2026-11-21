//! The main window's side of tracks (S1.6): it gives the player the preferred languages from
//! the settings and follows changes to them, and cycles audio (B) and subtitles (V), saying
//! which track plays now. Until the player's on-screen display exists (Stage 2) that is a toast,
//! replacing the previous one so a held key doesn't stack them.

use gpui::{Context, Entity, Subscription};

use super::MainWindow;
use crate::engine::language;
use crate::engine::player::Player;
use crate::engine::tracks::auto::{SubtitleMode, TrackPreferences};
use crate::engine::tracks::{Cycled, TrackKind, Tracks};
use crate::settings::{self, Settings, SettingsStore};
use crate::ui::toast::{Level, Toast};

/// What the window keeps about tracks.
#[derive(Default)]
pub(super) struct TrackWatch {
    /// Re-applies the language settings to the player when they change.
    settings: Option<Subscription>,
    /// The toast that names the track cycled to last, while it may still show.
    toast: Option<u64>,
}

impl MainWindow {
    /// Gives a new `player` the track preferences, and follows later changes to them.
    pub(super) fn set_up_tracks(&mut self, player: &Entity<Player>, cx: &mut Context<Self>) {
        let wanted = preferences(settings::get(cx));
        player.update(cx, |player, _| player.set_track_preferences(wanted));
        let player = player.downgrade();
        self.tracks.settings = Some(cx.observe_global::<SettingsStore>(move |_, cx| {
            let wanted = preferences(settings::get(cx));
            // The player lives as long as the window that owns this subscription.
            let _ = player.update(cx, |player, _| player.set_track_preferences(wanted));
        }));
    }

    /// Selects the next track of `kind` and names it in a toast.
    pub(super) fn cycle_track(&mut self, kind: TrackKind, cx: &mut Context<Self>) {
        let Some((player, _)) = &self.player else { return };
        let message = player.update(cx, |player, cx| {
            let cycled = player.cycle_track(kind, cx);
            announcement(kind, cycled, player.tracks())
        });
        tracing::debug!("tracks: {message}");
        let previous = self.tracks.toast.take();
        let id = self.toasts.update(cx, |toasts, cx| {
            if let Some(previous) = previous {
                toasts.dismiss(previous, cx);
            }
            toasts.push(Toast::new(Level::Info, message), cx)
        });
        self.tracks.toast = Some(id);
    }
}

/// The track preferences in `settings`.
fn preferences(settings: &Settings) -> TrackPreferences {
    let subtitles = &settings.prefs.subtitles;
    TrackPreferences {
        audio_languages: language::parse_list(&settings.prefs.audio.preferred_languages),
        subtitle_languages: language::parse_list(&subtitles.preferred_languages),
        subtitles: if subtitles.enabled { SubtitleMode::Show } else { SubtitleMode::ForcedOnly },
    }
}

/// What the toast says after cycling `kind`: "Audio: English — AC3 5.1", "Subtitles: Off".
fn announcement(kind: TrackKind, cycled: Cycled, tracks: &Tracks) -> String {
    let what = match kind {
        TrackKind::Audio => "Audio",
        TrackKind::Video => "Video",
        TrackKind::Subtitle => "Subtitles",
    };
    match cycled {
        Cycled::NoTracks if kind == TrackKind::Subtitle => "No subtitles".to_owned(),
        Cycled::NoTracks => format!("No {} tracks", what.to_lowercase()),
        Cycled::Off => format!("{what}: Off"),
        Cycled::Track(id) => format!("{what}: {}", tracks.label(id).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::language::Language;
    use crate::engine::tracks::{Track, TrackId};

    #[test]
    fn announcements_name_the_new_track() {
        let tracks = Tracks {
            all: vec![Track {
                language: Language::parse("eng"),
                codec: Some("AC3".into()),
                channels: Some(6),
                ..Track::new(TrackId(1), TrackKind::Audio)
            }],
            ..Tracks::default()
        };
        let audio = announcement(TrackKind::Audio, Cycled::Track(TrackId(1)), &tracks);
        assert_eq!(audio, "Audio: English — AC3 5.1");
        assert_eq!(announcement(TrackKind::Subtitle, Cycled::Off, &tracks), "Subtitles: Off");
        assert_eq!(announcement(TrackKind::Subtitle, Cycled::NoTracks, &tracks), "No subtitles");
        assert_eq!(announcement(TrackKind::Audio, Cycled::NoTracks, &tracks), "No audio tracks");
    }

    #[test]
    fn turning_subtitles_off_keeps_forced_ones() {
        let mut settings = Settings::default();
        settings.prefs.subtitles.enabled = false;
        settings.prefs.audio.preferred_languages = vec!["jpn".into(), "und".into()];
        let wanted = preferences(&settings);
        assert_eq!(wanted.subtitles, SubtitleMode::ForcedOnly);
        assert_eq!(wanted.audio_languages, [Language::parse("ja").unwrap()]);
    }
}
