//! The main window's side of playback history (S1.7): each open begins a history [`Session`],
//! playback saves its position every few seconds and on pause, stop, error and the end (the
//! session saves the last one when the window closes or the app quits), and media left partway
//! resumes as the "Continue playback" setting says: at once, after a "Resume from 12:34?"
//! toast, or never.
//!
//! It also runs Mark as Watched, Mark as Unwatched and Clear History (S1.7.5), and offers to
//! clear what was saved when "Save recently played items" is turned off.

use std::time::{Duration, Instant};

use gpui::{App, Context, Div, Subscription, prelude::*};

use super::MainWindow;
use crate::app::media::Media;
use crate::db::media::Saved;
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::state::PlaybackState;
use crate::engine::values::SeekTarget;
use crate::history::policy::{self, Rules, SAVE_INTERVAL};
use crate::history::{History, Session, SessionId};
use crate::keymap::actions::{ClearHistory, MarkUnwatched, MarkWatched};
use crate::opening::recent;
use crate::settings::{self, ResumePolicy, SettingsStore};
use crate::ui::toast::{Level, Toast};

/// What the window keeps about history.
#[derive(Default)]
pub(super) struct HistoryWatch {
    /// The playback being recorded, while history is on.
    session: Option<Session>,
    /// When the session last saved.
    last_save: Option<Instant>,
    /// Whether the session counted its play.
    counted: bool,
    /// The "Resume from …?" toast, while it may still show.
    resume_toast: Option<u64>,
    /// Whether history was on at the last settings change.
    was_on: bool,
    /// Offers to clear history when it is turned off.
    settings: Option<Subscription>,
    /// Ends the session when the app quits.
    quit: Option<Subscription>,
    /// What a restored session opened at its saved position (S3.4): its open offers no resume.
    restored: Option<Media>,
}

impl MainWindow {
    /// Follows the privacy settings and ends the session when the app quits.
    pub(super) fn set_up_history(&mut self, cx: &mut Context<Self>) {
        self.history.was_on = history_on(cx);
        self.history.settings = Some(cx.observe_global::<SettingsStore>(|this, cx| {
            let on = history_on(cx);
            if std::mem::replace(&mut this.history.was_on, on) && !on {
                this.history_turned_off(cx);
            }
        }));
        self.history.quit = Some(cx.on_app_quit(|this, cx| {
            this.end_history(cx);
            async {}
        }));
    }

    /// Adds the history commands' handlers.
    pub(super) fn history_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &MarkWatched, _, cx| this.mark_watched(true, cx)))
            .on_action(cx.listener(|this, _: &MarkUnwatched, _, cx| this.mark_watched(false, cx)))
            .on_action(cx.listener(|this, _: &ClearHistory, _, cx| {
                this.discard_history();
                clear_history(cx);
            }))
    }

    /// Records what the player does.
    pub(super) fn on_history_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        match event {
            PlayerEvent::StateChanged(PlaybackState::Opening) => self.begin_history(cx),
            PlayerEvent::StateChanged(PlaybackState::Playing) => self.count_play(),
            PlayerEvent::StateChanged(PlaybackState::Paused) => self.track_progress(true, cx),
            PlayerEvent::StateChanged(PlaybackState::Stopped | PlaybackState::Error(_)) => {
                self.save_history();
            }
            PlayerEvent::EndReached => self.finish_history(cx),
            PlayerEvent::PositionChanged(_) => self.track_progress(false, cx),
            _ => {}
        }
    }

    /// `media` opens where a restored session left it, so its open offers no resume.
    pub(super) fn skip_resume_offer(&mut self, media: Media) {
        self.history.restored = Some(media);
    }

    /// Ends the last session and begins one for what the player opens, then offers to resume.
    fn begin_history(&mut self, cx: &mut Context<Self>) {
        self.end_history(cx);
        let restored = self.history.restored.take();
        if !history_on(cx) {
            return;
        }
        let (Some(history), Some(player)) = (History::get(cx), self.player_entity()) else {
            return;
        };
        let Some(media) = player.read(cx).media().cloned() else { return };
        let offer = restored.as_ref() != Some(&media);
        let (session, saved) = history.begin(media);
        let id = session.id();
        self.history.session = Some(session);
        if !offer {
            return;
        }
        cx.spawn(async move |this, cx| {
            if let Ok(Some(saved)) = saved.await {
                this.update(cx, |this, cx| this.offer_resume(id, saved, cx)).ok();
            }
        })
        .detach();
    }

    /// Saves and ends the session, and takes back its resume offer.
    fn end_history(&mut self, cx: &mut Context<Self>) {
        self.history.session = None;
        self.history.last_save = None;
        self.history.counted = false;
        if let Some(toast) = self.history.resume_toast.take() {
            self.toasts.update(cx, |toasts, cx| toasts.dismiss(toast, cx));
        }
    }

    /// Ends the session without saving what it hadn't saved yet.
    fn discard_history(&mut self) {
        if let Some(session) = self.history.session.take() {
            session.discard();
        }
    }

    fn count_play(&mut self) {
        let Some(session) = &self.history.session else { return };
        if !std::mem::replace(&mut self.history.counted, true) {
            session.played();
        }
    }

    /// Takes the player's position into the session, and saves it if `now` or a save is due.
    fn track_progress(&mut self, now: bool, cx: &mut Context<Self>) {
        let Some(player) = self.player_entity() else { return };
        let player = player.read(cx);
        // Opening, stopped and ended players report positions that aren't where the user left.
        let tracking = matches!(
            player.state(),
            PlaybackState::Buffering | PlaybackState::Playing | PlaybackState::Paused
        );
        let rules = Rules::from_settings(settings::get(cx));
        let progress = rules.progress(player.position(), player.duration());
        let Some(session) = &mut self.history.session else { return };
        if let Some(progress) = progress.filter(|_| tracking) {
            session.update(progress);
        }
        let due = self.history.last_save.is_none_or(|last| last.elapsed() >= SAVE_INTERVAL);
        if now || due {
            self.save_history();
        }
    }

    fn save_history(&mut self) {
        if let Some(session) = &mut self.history.session {
            session.save();
            self.history.last_save = Some(Instant::now());
        }
    }

    fn finish_history(&mut self, cx: &mut Context<Self>) {
        let duration = self.player_entity().and_then(|player| player.read(cx).duration());
        if let Some(session) = &mut self.history.session {
            session.update(Rules::finished(duration));
        }
        self.save_history();
    }

    /// Resumes, or offers to, when session `id` is still running and its media was left
    /// partway.
    fn offer_resume(&mut self, id: SessionId, saved: Saved, cx: &mut Context<Self>) {
        if !self.is_session(id) {
            return;
        }
        let Some(player) = self.player_entity() else { return };
        let settings = settings::get(cx);
        let rules = Rules::from_settings(settings);
        let duration = player.read(cx).duration().or(saved.duration);
        let Some(at) = rules.resume_point(saved.position, duration) else { return };
        match settings.prefs.player.resume {
            ResumePolicy::Always => self.resume_at(id, at, cx),
            ResumePolicy::Ask => self.ask_to_resume(id, at, cx),
            ResumePolicy::Never => {}
        }
    }

    fn ask_to_resume(&mut self, id: SessionId, at: Duration, cx: &mut Context<Self>) {
        let main = cx.entity().downgrade();
        let text = format!("Resume from {}?", policy::clock(at));
        let toast = Toast::new(Level::Info, text).action("Resume", move |_, cx| {
            main.update(cx, |this, cx| this.resume_at(id, at, cx)).ok();
        });
        let toast = self.toasts.update(cx, |toasts, cx| toasts.push(toast, cx));
        self.history.resume_toast = Some(toast);
    }

    fn resume_at(&mut self, id: SessionId, at: Duration, cx: &mut Context<Self>) {
        self.history.resume_toast = None;
        if let Some(player) = self.player_entity().filter(|_| self.is_session(id)) {
            player.update(cx, |player, cx| player.seek(SeekTarget::Absolute(at), cx));
        }
    }

    fn is_session(&self, id: SessionId) -> bool {
        self.history.session.as_ref().is_some_and(|session| session.id() == id)
    }

    /// Marks what's playing watched or unwatched. This playback stops recording, so its
    /// position doesn't undo the mark.
    fn mark_watched(&mut self, watched: bool, cx: &mut Context<Self>) {
        let media = self.player_entity().and_then(|player| player.read(cx).media().cloned());
        let (Some(media), Some(history)) = (media, History::get(cx)) else { return };
        self.discard_history();
        history.set_watched(media, watched);
    }

    fn history_turned_off(&mut self, cx: &mut Context<Self>) {
        self.discard_history();
        let toast = Toast::new(Level::Info, "History is off. Clear what was already saved?")
            .action("Clear History", |_, cx| clear_history(cx));
        self.show_toast(toast, cx);
    }

    fn player_entity(&self) -> Option<gpui::Entity<Player>> {
        self.player.as_ref().map(|(player, _)| player.clone())
    }
}

/// Whether "Save recently played items" is on.
fn history_on(cx: &App) -> bool {
    settings::get(cx).prefs.privacy.save_recently_played
}

/// Forgets the playback history and the recent media list.
fn clear_history(cx: &mut App) {
    if let Some(history) = History::get(cx) {
        history.clear();
    }
    recent::clear(cx);
}
