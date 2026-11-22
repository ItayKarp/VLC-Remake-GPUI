//! The main window's side of tracks (S1.6): it gives the player the preferred languages, default
//! tracks and the options new media opens with (S5.7, S5.8.1) from the settings and follows changes to
//! them, and cycles audio (B) and subtitles (V), saying
//! which track plays now over the video (S2.10.2); a newer message replaces the last, so a held
//! key doesn't stack them.

use gpui::{Context, Entity, Subscription};

use super::MainWindow;
use crate::engine::language::{self, Language};
use crate::engine::media_options::{Deinterlacing, MediaOptions};
use crate::engine::player::Player;
use crate::engine::tracks::auto::{SubtitleMode, TrackPreferences};
use crate::engine::tracks::{Cycled, TrackKind, Tracks};
use crate::settings::{self, Settings, SettingsStore};

/// What the window keeps about tracks.
#[derive(Default)]
pub(super) struct TrackWatch {
    /// Re-applies the track and media settings to the player when they change.
    settings: Option<Subscription>,
}

impl MainWindow {
    /// Gives a new `player` the track preferences and media options, and follows later changes
    /// to them.
    pub(super) fn set_up_tracks(&mut self, player: &Entity<Player>, cx: &mut Context<Self>) {
        let settings = settings::get(cx).clone();
        player.update(cx, |player, cx| apply(&settings, player, cx));
        let player = player.downgrade();
        self.tracks.settings = Some(cx.observe_global::<SettingsStore>(move |_, cx| {
            let settings = settings::get(cx).clone();
            // The player lives as long as the window that owns this subscription.
            let _ = player.update(cx, |player, cx| apply(&settings, player, cx));
        }));
    }

    /// Selects the next track of `kind` and names it over the video.
    pub(super) fn cycle_track(&mut self, kind: TrackKind, cx: &mut Context<Self>) {
        let Some((player, _)) = &self.player else { return };
        let message = player.update(cx, |player, cx| {
            let cycled = player.cycle_track(kind, cx);
            announcement(kind, cycled, player.tracks())
        });
        tracing::debug!("tracks: {message}");
        self.show_osd(message, cx);
    }
}

/// Gives `player` what `settings` say about new media's tracks and options, and whether styled
/// subtitle files keep their own look (Embedded Sub Format, live).
fn apply(settings: &Settings, player: &mut Player, cx: &mut Context<Player>) {
    player.set_track_preferences(preferences(settings));
    player.set_media_options(media_options(settings));
    player.set_file_styles(settings.prefs.subtitles.file_styles, cx);
}

/// The track preferences in `settings`.
fn preferences(settings: &Settings) -> TrackPreferences {
    let subtitles = &settings.prefs.subtitles;
    let player = &settings.prefs.player;
    TrackPreferences {
        audio_languages: language::parse_list(&settings.prefs.audio.preferred_languages),
        subtitle_languages: language::parse_list(&subtitles.preferred_languages),
        subtitles: if subtitles.enabled { SubtitleMode::Show } else { SubtitleMode::ForcedOnly },
        audio_track: position(player.default_audio_track),
        subtitle_track: position(player.default_subtitle_track),
    }
}

/// A "Default … track" number (counted from 1, 0 for none) as a position among the tracks.
fn position(number: u32) -> Option<usize> {
    usize::try_from(number.checked_sub(1)?).ok()
}

/// The options new media opens with, in `settings`.
fn media_options(settings: &Settings) -> MediaOptions {
    let player = &settings.prefs.player;
    let video = &settings.prefs.video;
    MediaOptions {
        max_lines: player.video_quality.max_lines(),
        menu_language: Language::parse(&player.menu_language),
        precise_seeks: player.seek_precisely,
        video: video.enabled,
        audio: settings.prefs.audio.enabled,
        picture: video.picture.into(),
        deinterlace: Deinterlacing { mode: video.deinterlace, method: video.deinterlace_method },
        subtitles: super::subtitle_style::loading(&settings.prefs.subtitles),
    }
}

/// What the message says after cycling `kind`: "Audio: English — AC3 5.1", "Subtitles: Off".
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
    use crate::engine::tracks::{Track, TrackId};
    use crate::settings::VideoQuality;

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

    #[test]
    fn track_numbers_count_from_one_and_zero_is_none() {
        let mut settings = Settings::default();
        settings.prefs.player.default_audio_track = 2;
        let wanted = preferences(&settings);
        assert_eq!((wanted.audio_track, wanted.subtitle_track), (Some(1), None));
    }

    #[test]
    fn media_options_follow_the_playback_settings() {
        let mut settings = Settings::default();
        let mut defaults = media_options(&settings);
        assert!(defaults.subtitles.converted.is_some(), "converted files go to the cache");
        defaults.subtitles.converted = None;
        assert_eq!(defaults, MediaOptions::default());
        settings.prefs.player.video_quality = VideoQuality::High;
        settings.prefs.player.menu_language = "fra".into();
        settings.prefs.player.seek_precisely = false;
        let options = media_options(&settings);
        assert_eq!(options.max_lines, Some(720));
        assert_eq!(options.menu_language, Language::parse("fr"));
        assert!(!options.precise_seeks);
    }
}
