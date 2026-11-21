//! Playing the queue (S3.1): opened items replace the queue, are added to its end or go right
//! after the current item; the player then moves through it by itself.
//!
//! At an item's end the next one plays, as Repeat and Shuffle say (S3.3), unless "Stop after
//! each item" holds it; when the queue runs out, "When playback finishes" decides. An item that
//! fails is skipped with a toast ("Couldn't play X, skipped"), and after
//! [`MAX_CONSECUTIVE_FAILURES`] failures in a row the queue stops, so a broken list (a
//! disconnected drive, deleted files) isn't spun through, even under Repeat All.

use gpui::{Action, Context, Window};

use super::MainWindow;
use super::opening::Target;
use crate::app::media::Media;
use crate::app::request::Placement;
use crate::engine::values::Start;
use crate::keymap::actions::Quit;
use crate::opening::Resolved;
use crate::queue::{AfterError, MAX_CONSECUTIVE_FAILURES};
use crate::settings::{self, WhenFinished};
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Puts the media `resolved` found in the queue at `target` and plays it from its start
    /// item, unless it was added while something else plays.
    pub(super) fn play_resolved(
        &mut self,
        resolved: &mut Resolved,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let media = std::mem::take(&mut resolved.media);
        let replace = target == Target::Placed(Placement::Replace);
        let first_new = match target {
            Target::Placed(Placement::Replace) => {
                self.queue.replace(media);
                self.skips.played();
                0
            }
            Target::Placed(Placement::Enqueue) => self.queue.append(media),
            Target::Placed(Placement::PlayNext) => self.queue.insert_next(media),
            Target::Before(id) => {
                let at = id.and_then(|id| self.queue.index_of(id)).unwrap_or(self.queue.len());
                self.queue.insert_at(at, media)
            }
        };
        if replace || !self.is_playing(cx) {
            self.play(first_new + resolved.start, window, cx);
        }
        cx.notify();
    }

    /// Plays item `index` of the queue.
    pub(super) fn play(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.play_from(index, Start::default(), window, cx);
    }

    /// Opens item `index` of the queue where `start` says: playing from its start, or paused
    /// at a restored session's position (S3.4).
    pub(super) fn play_from(
        &mut self,
        index: usize,
        start: Start,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(media) = self.queue.set_current(index).cloned() else {
            return;
        };
        let player = self.player(window, cx);
        player.update(cx, |player, cx| player.open_at(media, start, cx));
        // The video replaces the empty state and its focused Open button; with nothing focused
        // the player's keys would reach nothing.
        if self.open_button.is_focused(window) || window.focused(cx).is_none() {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// The current item ended: the next one plays (the same one under Repeat One), unless
    /// "Stop after each item" is on; Play then goes on. At the end of the queue, "When playback
    /// finishes" says what happens.
    pub(super) fn advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prefs = &settings::get(cx).prefs.player;
        let (stop_after_each, when_finished) = (prefs.stop_after_each_item, prefs.when_finished);
        match self.queue.after_end() {
            Some(next) if !stop_after_each => self.play(next, window, cx),
            Some(_) => {}
            None => match when_finished {
                WhenFinished::Stop => {}
                WhenFinished::Quit => window.dispatch_action(Quit.boxed_clone(), cx),
            },
        }
    }

    /// The current item couldn't play: skips to the next one, unless too many failed in a row.
    /// The last item's error stays on screen instead, and so does that of an item loaded paused
    /// (a restored session, S3.4): skipping would start playing what nobody asked to play.
    pub(super) fn skip_failed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let wanted_to_play = self.loaded_player(cx).is_some_and(|p| p.read(cx).wants_to_play());
        let Some(next) = self.queue.next().filter(|_| wanted_to_play) else { return };
        let name = self.playing_name(cx);
        let toast = match self.skips.failed() {
            AfterError::Skip => {
                self.play(next, window, cx);
                format!("Couldn't play “{name}”, skipped.")
            }
            AfterError::GiveUp => {
                format!("Stopped: {MAX_CONSECUTIVE_FAILURES} items in a row couldn't play.")
            }
        };
        self.show_toast(Toast::new(Level::Error, toast), cx);
    }

    /// The name of what the player has.
    fn playing_name(&self, cx: &Context<Self>) -> String {
        let player = self.player.as_ref().map(|(player, _)| player.read(cx));
        player.and_then(|player| player.media().map(Media::display_name)).unwrap_or_default()
    }
}
