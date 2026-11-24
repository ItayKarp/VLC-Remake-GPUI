//! The main window's side of scrobbling (S8.17.1): it follows what plays, measures how long each
//! track is heard ([`Listening`]), and hands a track to [`scrobble`] once it counts, once per
//! play. A stream whose tags change as it plays (a radio station naming each song) starts a new
//! track at each change. Media "Never remember items matching" names isn't scrobbled.
//!
//! Why submitting stopped is shown once, as a toast.

use gpui::{App, Context, Subscription};

use super::MainWindow;
use crate::db::scrobbles::Play;
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

impl MainWindow {
    /// Shows why submitting to last.fm stopped, now and whenever it stops again.
    pub(super) fn set_up_scrobbling(&mut self, cx: &mut Context<Self>) {
        self.show_scrobble_problem(cx);
        let problems = cx.observe_global::<Scrobbler>(Self::show_scrobble_problem);
        self.scrobbling = Scrobbling { _problems: Some(problems), ..Scrobbling::default() };
    }

    /// Follows what plays and how long.
    pub(super) fn on_scrobble_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        match event {
            PlayerEvent::StateChanged(PlaybackState::Opening) => self.end_track(),
            PlayerEvent::StateChanged(PlaybackState::Playing) => self.begin_track(cx),
            PlayerEvent::StateChanged(_) => self.scrobbling.listening.interrupted(),
            PlayerEvent::MetadataChanged => self.retag_track(cx),
            PlayerEvent::PositionChanged(position) => {
                self.scrobbling.listening.at(*position);
                self.scrobble_if_heard(cx);
            }
            _ => {}
        }
    }

    /// Starts listening to what plays, unless it is already the track.
    fn begin_track(&mut self, cx: &App) {
        self.scrobbling.listening.interrupted();
        if self.scrobbling.track.is_none() {
            self.scrobbling.track = self.playing_track(cx);
        }
    }

    /// The tags changed: they arrived for the track playing, or a stream names a new song,
    /// which starts a new track.
    fn retag_track(&mut self, cx: &App) {
        let now = self.playing_track(cx);
        let same = |a: &Play, b: &Play| a.artist == b.artist && a.title == b.title;
        match &self.scrobbling.track {
            Some(track) if now.as_ref().is_some_and(|now| same(track, now)) => {}
            Some(_) => {
                self.end_track();
                self.scrobbling.track = now;
            }
            None => self.scrobbling.track = now,
        }
    }

    fn end_track(&mut self) {
        self.scrobbling.track = None;
        self.scrobbling.sent = false;
        self.scrobbling.listening = Listening::default();
    }

    /// Hands the track over once it counts as played.
    fn scrobble_if_heard(&mut self, cx: &mut Context<Self>) {
        let length = self.player.as_ref().and_then(|(player, _)| player.read(cx).duration());
        if self.scrobbling.sent || !self.scrobbling.listening.counts(length) {
            return;
        }
        let Some(mut play) = self.scrobbling.track.clone() else { return };
        self.scrobbling.sent = true;
        play.duration_s = length.map(|length| length.as_secs() as u32);
        scrobble::scrobble(play, cx);
    }

    /// What plays as a track to scrobble: it needs an artist and a title, and media that isn't
    /// excluded from history.
    fn playing_track(&self, cx: &App) -> Option<Play> {
        if !crate::settings::get(cx).prefs.privacy.lastfm.submit {
            return None;
        }
        let (player, _) = self.player.as_ref()?;
        let player = player.read(cx);
        if exclusions::excluded(player.media()?, cx) {
            return None;
        }
        let tags = player.metadata();
        Some(Play {
            artist: tag(tags.artist.as_ref())?,
            title: tag(tags.title.as_ref())?,
            album: tag(tags.album.as_ref()),
            duration_s: None,
            started_s: crate::db::now_ms() / 1000,
        })
    }

    fn show_scrobble_problem(&mut self, cx: &mut Context<Self>) {
        if let Some(problem) = scrobble::take_problem(cx) {
            self.show_toast(Toast::new(Level::Warning, problem.message()), cx);
        }
    }
}

/// A tag's text, trimmed; `None` when it says nothing.
fn tag(tag: Option<&String>) -> Option<String> {
    let text = tag?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
