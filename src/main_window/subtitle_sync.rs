//! The main window's side of subtitle synchronization (S6.3): the delay keys (G and H, as in
//! VLC), the Subtitle menu's delay items and a line dragged on the seek bar's subtitle timeline
//! (S6.6.3), said over the video; and remembering each subtitle track's sync per media (S6.3.4),
//! restored when the media opens again.
//!
//! Syncs are remembered with the history: not while "Save recently played items" is off, nor
//! for media its exclusions name, and Clear History forgets them.

use gpui::{App, Context};

use super::MainWindow;
use super::audio_tracks::Shift;
use crate::app::media::Media;
use crate::db::subtitle_sync::{self, SavedSync};
use crate::engine::subtitle_sync::{SubtitleKey, SubtitleSync};
use crate::engine::values::Offset;
use crate::history::History;
use crate::privacy::exclusions;
use crate::settings::{self, stored_path};

/// How far one press of G or H, or one Increase or Decrease Delay, moves the subtitles.
const DELAY_STEP_MS: i64 = 50;

impl MainWindow {
    /// Moves the subtitles one step `shift` and says where they are now.
    pub(super) fn shift_subtitle_delay(&mut self, shift: Shift, cx: &mut Context<Self>) {
        let step = match shift {
            Shift::Earlier => -DELAY_STEP_MS,
            Shift::Later => DELAY_STEP_MS,
        };
        let Some(player) = self.loaded_player(cx) else { return };
        let sync = player.read(cx).subtitle_sync();
        let delay = Offset::from_millis(sync.delay().as_millis() + step);
        self.set_subtitle_sync(sync.with_delay(delay), cx);
    }

    /// Sets the showing subtitle track's sync, says the delay over the video, and remembers it
    /// for the media. With no subtitle showing, says so instead.
    pub(super) fn set_subtitle_sync(&mut self, sync: SubtitleSync, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let before = player.read(cx).subtitle_sync();
        let Some(key) = player.update(cx, |player, cx| player.set_subtitle_sync(sync, cx)) else {
            self.show_osd("No subtitles showing", cx);
            return;
        };
        let sync = player.read(cx).subtitle_sync();
        if sync.delay() != before.delay() {
            self.show_osd(delay_message(sync.delay()), cx);
        }
        if let Some(media) = player.read(cx).media().cloned() {
            remember(&media, &key, sync, cx);
        }
    }

    /// Moves the showing subtitle's delay to `delay` while one of its lines is dragged on the
    /// seek bar (S6.6.3), saying it over the video. It is remembered once let go
    /// ([`Self::remember_subtitle_sync`]), not at every step.
    pub(super) fn drag_subtitle_delay(&mut self, delay: Offset, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let sync = player.read(cx).subtitle_sync();
        if sync.delay() == delay {
            return;
        }
        let moved =
            player.update(cx, |player, cx| player.set_subtitle_sync(sync.with_delay(delay), cx));
        if moved.is_some() {
            let shown = player.read(cx).subtitle_sync().delay();
            self.show_osd(delay_message(shown), cx);
        }
    }

    /// Remembers the showing subtitle track's sync for the media, as it is now.
    pub(super) fn remember_subtitle_sync(&self, cx: &App) {
        let Some(player) = self.loaded_player(cx) else { return };
        let player = player.read(cx);
        let (Some(media), Some(key)) = (player.media(), player.subtitle_key()) else { return };
        remember(media, &key, player.subtitle_sync(), cx);
    }

    /// Hands the player the syncs remembered for the media it opens (a player event).
    pub(super) fn restore_subtitle_syncs(&self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let Some(media) = player.read(cx).media().cloned() else { return };
        if !remembers(&media, cx) {
            return;
        }
        let Some(history) = History::get(cx) else { return };
        let location = media_location(&media);
        let saved = history
            .query("subtitle sync", move |connection| subtitle_sync::load(connection, &location));
        cx.spawn(async move |_, cx| {
            let Ok(Some(rows)) = saved.await else { return };
            let syncs: Vec<_> =
                rows.iter().filter_map(|(key, saved)| restored(key, saved)).collect();
            if syncs.is_empty() {
                return;
            }
            player.update(cx, |player, cx| player.restore_subtitle_syncs(&media, syncs, cx));
        })
        .detach();
    }
}

/// Writes `sync` for the track `key` of `media`, when its history is kept.
fn remember(media: &Media, key: &SubtitleKey, sync: SubtitleSync, cx: &App) {
    if !remembers(media, cx) {
        return;
    }
    let Some(history) = History::get(cx) else { return };
    let (location, subtitle) = (media_location(media), subtitle_text(key));
    let saved = (!sync.is_default()).then(|| SavedSync {
        delay_ms: sync.delay().as_millis(),
        speed: f64::from(sync.speed()),
        duration_factor: f64::from(sync.duration_factor()),
    });
    history.with_database(move |connection| {
        if let Err(error) = subtitle_sync::save(connection, &location, &subtitle, saved) {
            tracing::warn!("subtitle sync: couldn't save it: {error}");
        }
    });
}

/// Whether what is played of `media` is remembered.
pub(super) fn remembers(media: &Media, cx: &App) -> bool {
    settings::get(cx).prefs.privacy.save_recently_played && !exclusions::excluded(media, cx)
}

/// How the table names `media`: its stored path, its URL, or its device's stored form.
pub(super) fn media_location(media: &Media) -> String {
    media.to_stored()
}

/// How the table names a subtitle track: `file:` and its stored path, or `stream:` and its id.
fn subtitle_text(key: &SubtitleKey) -> String {
    match key {
        SubtitleKey::File(path) => format!("file:{}", stored_path::encode(path)),
        SubtitleKey::Stream(id) => format!("stream:{id}"),
    }
}

/// A remembered row as the player takes it; `None` for one this build can't read.
fn restored(text: &str, saved: &SavedSync) -> Option<(SubtitleKey, SubtitleSync)> {
    let key = if let Some(path) = text.strip_prefix("file:") {
        SubtitleKey::File(stored_path::decode(path)?)
    } else {
        SubtitleKey::Stream(text.strip_prefix("stream:")?.parse().ok()?)
    };
    let sync = SubtitleSync::default()
        .with_delay(Offset::from_millis(saved.delay_ms))
        .with_speed(saved.speed as f32)
        .with_duration_factor(saved.duration_factor as f32);
    Some((key, sync))
}

/// The OSD's text for a subtitle delay: "Subtitle delay: −150 ms".
pub(super) fn delay_message(delay: Offset) -> String {
    match delay.as_millis() {
        0 => "Subtitle delay: 0 ms".to_owned(),
        millis if millis < 0 => format!("Subtitle delay: −{} ms", millis.unsigned_abs()),
        millis => format!("Subtitle delay: +{millis} ms"),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn keys_read_back_as_they_were_written() {
        let saved = SavedSync { delay_ms: -4_000, speed: 1.5, duration_factor: 2. };
        for key in
            [SubtitleKey::File(PathBuf::from("/movies/Movie.en.srt")), SubtitleKey::Stream(3)]
        {
            let (back, sync) = restored(&subtitle_text(&key), &saved).expect("it reads");
            assert_eq!(back, key);
            assert_eq!(sync.delay().as_millis(), -4_000);
        }
        assert_eq!(restored("chapter:1", &saved), None);
    }

    #[test]
    fn the_delay_reads_with_its_sign() {
        assert_eq!(delay_message(Offset::from_millis(0)), "Subtitle delay: 0 ms");
        assert_eq!(delay_message(Offset::from_millis(-150)), "Subtitle delay: −150 ms");
        assert_eq!(delay_message(Offset::from_millis(4_050)), "Subtitle delay: +4050 ms");
    }
}
