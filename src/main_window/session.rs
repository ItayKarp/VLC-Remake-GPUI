//! The main window's side of session restore (S3.4): it saves the queue, its current item and
//! position, Repeat and Shuffle as they change and when the app quits, and the first window of a
//! launch brings them back paused (see [`crate::session::restore`]).
//!
//! It all follows two settings: "Continue where you left off" (`prefs.player.restore_session`)
//! and, because the queue is a list of what was played, "Save recently played items"
//! (`prefs.privacy.save_recently_played`). With either off, nothing is saved or restored, and
//! what was saved is forgotten.
//!
//! With multiple instances allowed (S0.6.1), the instance that saves last wins.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::{App, AppContext as _, Context, Subscription, Task, Window};

use super::MainWindow;
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::engine::state::PlaybackState;
use crate::engine::values::Start;
use crate::session::cadence::{self, DEBOUNCE, Key, When};
use crate::session::restore::{self, Load};
use crate::session::{SessionStore, Snapshot};
use crate::settings;
use crate::ui::toast::{Level, Toast};

/// What the window keeps about saving the session.
#[derive(Default)]
pub(super) struct SessionWatch {
    /// What the last save recorded, and when.
    written: Key,
    written_at: Option<Instant>,
    /// The save waiting out [`DEBOUNCE`].
    pending: Option<Task<()>>,
    /// Whether saving was on at the last change.
    enabled: bool,
    /// Follows the window's changes.
    changes: Option<Subscription>,
    /// Saves when the app quits.
    quit: Option<Subscription>,
}

impl MainWindow {
    /// Saves the session whenever the window changes and when the app quits.
    pub(super) fn set_up_session(&mut self, cx: &mut Context<Self>) {
        self.session.enabled = saving_on(cx);
        self.session.changes = Some(cx.observe_self(Self::session_changed));
        self.session.quit = Some(cx.on_app_quit(|this, cx| {
            // Quitting isn't a crash: what was playing may load again next time.
            let snapshot = Snapshot { playing: false, ..this.snapshot(cx) };
            this.write_session(snapshot, cx);
            async {}
        }));
    }

    /// Brings back the last session, unless `request` replaces the queue anyway. Added items
    /// (`--enqueue`) join the restored queue.
    pub(super) fn restore_session(
        &mut self,
        request: &OpenRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(saved) = SessionStore::take_saved(cx) else { return };
        let replaced = request.placement == Placement::Replace && !request.items.is_empty();
        if replaced || !saving_on(cx) || saved.media.is_empty() {
            return;
        }
        let restored = restore::plan(saved, Path::exists);
        let session = restored.session;
        self.queue.replace(session.media);
        self.queue.set_repeat(session.repeat);
        if let Some(index) = session.current {
            self.queue.set_current(index);
        }
        self.queue.set_shuffle(session.shuffle);
        match (restored.load, session.current) {
            (Load::Paused(at), Some(index)) => {
                self.skip_resume_offer(self.queue.entries()[index].media.clone());
                self.play_from(index, Start { at, paused: true }, window, cx);
            }
            (Load::Crashed(media), _) => self.tell_about_crash(&media, cx),
            _ => {}
        }
        self.find_missing(cx);
        self.session.written = self.session_key(cx);
    }

    /// The window changed: saves the session when the change is worth it, or forgets the
    /// session when saving was just turned off.
    fn session_changed(&mut self, cx: &mut Context<Self>) {
        let enabled = saving_on(cx);
        let was_enabled = std::mem::replace(&mut self.session.enabled, enabled);
        if !enabled {
            self.session.pending = None;
            if was_enabled && let Some(store) = SessionStore::get(cx) {
                store.forget();
            }
            return;
        }
        let key = self.session_key(cx);
        let since = self.session.written_at.map_or(Duration::MAX, |at| at.elapsed());
        match cadence::when(self.session.written, key, since) {
            When::Now => {
                self.session.pending = None;
                self.write_session(self.snapshot(cx), cx);
            }
            When::Soon if self.session.pending.is_none() => self.save_session_soon(cx),
            When::Soon | When::Not => {}
        }
    }

    /// Saves after [`DEBOUNCE`], whatever the session is then.
    fn save_session_soon(&mut self, cx: &mut Context<Self>) {
        self.session.pending = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(DEBOUNCE).await;
            this.update(cx, |this, cx| {
                this.session.pending = None;
                this.write_session(this.snapshot(cx), cx);
            })
            .ok();
        }));
    }

    fn write_session(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        if !saving_on(cx) {
            return;
        }
        self.session.written = self.session_key(cx);
        self.session.written_at = Some(Instant::now());
        if let Some(store) = SessionStore::get(cx) {
            store.save(snapshot);
        }
    }

    /// What a save would record now, for telling whether it changed.
    fn session_key(&self, cx: &App) -> Key {
        let (playing, position) = self.playback(cx);
        Key {
            revision: self.queue.revision(),
            current: self.queue.current_index(),
            repeat: self.queue.repeat(),
            shuffle: self.queue.is_shuffled(),
            playing,
            second: position.as_secs(),
        }
    }

    /// The session as it is now.
    fn snapshot(&self, cx: &App) -> Snapshot {
        let (playing, position) = self.playback(cx);
        Snapshot {
            media: self.queue.entries().iter().map(|entry| entry.media.clone()).collect(),
            current: self.queue.current_index(),
            position,
            repeat: self.queue.repeat(),
            shuffle: self.queue.is_shuffled(),
            playing,
        }
    }

    /// Whether the current item is playing, and where it is: the start unless it's loaded,
    /// since a stopped, finished or failed item plays from its start again.
    fn playback(&self, cx: &App) -> (bool, Duration) {
        let Some(player) = self.loaded_player(cx) else { return (false, Duration::ZERO) };
        let player = player.read(cx);
        if !player.state().is_active() {
            return (false, Duration::ZERO);
        }
        (player.wants_to_play(), player.position())
    }

    /// "VLC closed unexpectedly while playing X": the restored item waits for Play.
    fn tell_about_crash(&mut self, media: &Media, cx: &mut Context<Self>) {
        let text = format!(
            "VLC closed unexpectedly while playing “{}”, so it wasn't opened again.",
            media.display_name()
        );
        self.show_toast(Toast::new(Level::Warning, text), cx);
    }

    /// Greys the restored files that aren't there, checking off the UI thread (a long queue on
    /// an offline drive can take a while).
    fn find_missing(&mut self, cx: &mut Context<Self>) {
        let entries = self.queue.entries().iter();
        let files: Vec<PathBuf> =
            entries.filter_map(|entry| entry.media.as_path().map(PathBuf::from)).collect();
        let check = cx.background_spawn(async move {
            files.into_iter().filter(|path| !path.exists()).map(Media::File).collect()
        });
        cx.spawn(async move |this, cx| {
            let missing: HashSet<Media> = check.await;
            this.update(cx, |this, cx| {
                this.queue_panel.missing = missing;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Media that plays is there after all (a drive came back): it isn't greyed any more.
    pub(super) fn found_playing(&mut self, state: PlaybackState, cx: &App) {
        if state != PlaybackState::Playing || self.queue_panel.missing.is_empty() {
            return;
        }
        if let Some(media) = self.loaded_player(cx).and_then(|p| p.read(cx).media().cloned()) {
            self.queue_panel.missing.remove(&media);
        }
    }
}

#[cfg(test)]
mod tests;

/// Whether the session is saved and restored: "Continue where you left off" and "Save recently
/// played items" are both on.
fn saving_on(cx: &App) -> bool {
    let prefs = &settings::get(cx).prefs;
    prefs.player.restore_session && prefs.privacy.save_recently_played
}
