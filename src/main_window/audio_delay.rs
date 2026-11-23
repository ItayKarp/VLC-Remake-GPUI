//! Remembering each audio track's delay per media (S7.6.3), as subtitle syncs are (S6.3.4): a
//! delay set with J and K or the settings panel is saved for the media, and comes back when it
//! opens again. Delays are kept with the history: not while "Save recently played items" is off,
//! nor for media its exclusions name, and Clear History forgets them.
//!
//! A track is named by its stream id, so an audio file added to the media (S4.8.1), which isn't
//! added again on the next open, may hand its delay to a track that later takes its id.

use gpui::{App, Context};

use super::MainWindow;
use super::subtitle_sync::{media_location, remembers};
use crate::db::audio_delay;
use crate::engine::tracks::TrackId;
use crate::engine::values::Offset;
use crate::history::History;

impl MainWindow {
    /// Remembers the playing audio track's delay for the media, as it is now.
    pub(super) fn remember_audio_delay(&self, cx: &App) {
        let Some(player) = self.loaded_player(cx) else { return };
        let player = player.read(cx);
        let (Some(media), Some(track)) = (player.media(), player.audio_track()) else { return };
        if !remembers(media, cx) {
            return;
        }
        let Some(history) = History::get(cx) else { return };
        let (location, delay) = (media_location(media), player.audio_delay().as_millis());
        history.with_database(move |connection| {
            if let Err(error) = audio_delay::save(connection, &location, track.0, delay) {
                tracing::warn!("audio delay: couldn't save it: {error}");
            }
        });
    }

    /// Hands the player the delays remembered for the media it opens (a player event).
    pub(super) fn restore_audio_delays(&self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let Some(media) = player.read(cx).media().cloned() else { return };
        if !remembers(&media, cx) {
            return;
        }
        let Some(history) = History::get(cx) else { return };
        let location = media_location(&media);
        let saved = history
            .query("audio delay", move |connection| audio_delay::load(connection, &location));
        cx.spawn(async move |_, cx| {
            let Ok(Some(rows)) = saved.await else { return };
            if rows.is_empty() {
                return;
            }
            let delays = restored(rows);
            player.update(cx, |player, cx| player.restore_audio_delays(&media, delays, cx));
        })
        .detach();
    }
}

/// The stored rows as the player takes them.
fn restored(rows: Vec<(i32, i64)>) -> Vec<(TrackId, Offset)> {
    rows.into_iter().map(|(track, millis)| (TrackId(track), Offset::from_millis(millis))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_read_back_as_tracks_and_delays() {
        assert_eq!(restored(vec![(2, -300)]), [(TrackId(2), Offset::from_millis(-300))]);
    }
}
