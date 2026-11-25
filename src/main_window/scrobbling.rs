//! The main window's side of scrobbling (S8.17.1): it follows what plays, measures how long each
//! track is heard ([`Listening`]), and hands a track to [`scrobble`] once it counts, once per
//! play. A stream whose tags change as it plays (a radio station naming each song) starts a new
//! track at each change. Media "Never remember items matching" names isn't scrobbled.
//!
//! What to hand over is decided by [`Scrobbling::follow`] from the player's events alone; the
//! window only reads the player for it. Why submitting stopped is shown once, as a toast.

use std::time::Duration;

use gpui::{App, Context, Subscription};

use super::MainWindow;
use crate::app::media::Media;
use crate::db::scrobbles::Play;
use crate::engine::media_info::Metadata;
use crate::engine::player::PlayerEvent;
use crate::engine::state::PlaybackState;
use crate::privacy::exclusions;
use crate::scrobble::listening::Listening;
use crate::scrobble::{self, Scrobbler};
use crate::ui::toast::{Level, Toast};

/// The track being listened to.
#[derive(Default)]
pub(super) struct Scrobbling {
    /// Its tags and when it started.
    track: Option<Play>,
    /// Whether it was handed over already.
    sent: bool,
    /// How long it was heard.
    listening: Listening,
    /// Shows why submitting stopped.
    _problems: Option<Subscription>,
}

impl Scrobbling {
    /// Follows the player's `event`, for media of `length` whose track `playing` answers (asked
    /// only when needed). Answers the track to hand over, once it counts as played.
    fn follow(
        &mut self,
        event: &PlayerEvent,
        playing: impl FnOnce() -> Option<Play>,
        length: Option<Duration>,
    ) -> Option<Play> {
        match event {
            PlayerEvent::StateChanged(PlaybackState::Opening) => self.end_track(),
            PlayerEvent::StateChanged(PlaybackState::Playing) => self.begin_track(playing),
            PlayerEvent::StateChanged(_) => self.listening.interrupted(),
            PlayerEvent::MetadataChanged => self.retag_track(playing()),
            PlayerEvent::PositionChanged(position) => {
                self.listening.at(*position);
                return self.heard(length);
            }
            _ => {}
        }
        None
    }

    /// Starts listening to what plays, unless it is already the track.
    fn begin_track(&mut self, playing: impl FnOnce() -> Option<Play>) {
        self.listening.interrupted();
        if self.track.is_none() {
            self.track = playing();
        }
    }

    /// The tags changed to `now`'s: they arrived for the track playing, or a stream names a new
    /// song, which starts a new track.
    fn retag_track(&mut self, now: Option<Play>) {
        let same = |a: &Play, b: &Play| a.artist == b.artist && a.title == b.title;
        match &self.track {
            Some(track) if now.as_ref().is_some_and(|now| same(track, now)) => {}
            Some(_) => {
                self.end_track();
                self.track = now;
            }
            None => self.track = now,
        }
    }

    fn end_track(&mut self) {
        self.track = None;
        self.sent = false;
        self.listening = Listening::default();
    }

    /// The track, with its `length`, once it counts as played and wasn't handed over yet.
    fn heard(&mut self, length: Option<Duration>) -> Option<Play> {
        if self.sent || !self.listening.counts(length) {
            return None;
        }
        let mut play = self.track.clone()?;
        self.sent = true;
        play.duration_s = length.map(|length| length.as_secs() as u32);
        Some(play)
    }
}

impl MainWindow {
    /// Shows why submitting to last.fm stopped, now and whenever it stops again.
    pub(super) fn set_up_scrobbling(&mut self, cx: &mut Context<Self>) {
        self.show_scrobble_problem(cx);
        let problems = cx.observe_global::<Scrobbler>(Self::show_scrobble_problem);
        self.scrobbling = Scrobbling { _problems: Some(problems), ..Scrobbling::default() };
    }

    /// Follows what plays and how long, and hands a track over once it counts.
    pub(super) fn on_scrobble_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        let player = self.player.as_ref().map(|(player, _)| player);
        let length = player.and_then(|player| player.read(cx).duration());
        let playing = || {
            let player = player?.read(cx);
            track(player.media()?, player.metadata(), cx)
        };
        if let Some(play) = self.scrobbling.follow(event, playing, length) {
            scrobble::scrobble(play, cx);
        }
    }

    fn show_scrobble_problem(&mut self, cx: &mut Context<Self>) {
        if let Some(problem) = scrobble::take_problem(cx) {
            self.show_toast(Toast::new(Level::Warning, problem.message()), cx);
        }
    }
}

/// `media` with `tags` as a track to scrobble, starting now: it needs an artist and a title,
/// submitting turned on, and media that isn't excluded from history.
fn track(media: &Media, tags: &Metadata, cx: &App) -> Option<Play> {
    if !crate::settings::get(cx).prefs.privacy.lastfm.submit || exclusions::excluded(media, cx) {
        return None;
    }
    Some(Play {
        artist: tag(tags.artist.as_ref())?,
        title: tag(tags.title.as_ref())?,
        album: tag(tags.album.as_ref()),
        duration_s: None,
        started_s: crate::db::now_ms() / 1000,
    })
}

/// A tag's text, trimmed; `None` when it says nothing.
fn tag(tag: Option<&String>) -> Option<String> {
    let text = tag?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

#[cfg(test)]
mod tests;
