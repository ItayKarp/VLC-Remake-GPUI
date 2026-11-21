//! Playing the queue (S3.1): opened items replace the queue, are added to its end or go right
//! after the current item; the player then moves through it by itself.
//!
//! At an item's end the next one plays. An item that fails is skipped with a toast ("Couldn't
//! play X, skipped"), and after [`MAX_CONSECUTIVE_FAILURES`] failures in a row the queue
//! stops, so a broken list (a disconnected drive, deleted files) isn't spun through.

use gpui::{Context, Window};

use super::MainWindow;
use crate::app::media::Media;
use crate::app::request::Placement;
use crate::opening::Resolved;
use crate::queue::{AfterError, MAX_CONSECUTIVE_FAILURES};
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Puts the media `resolved` found in the queue as `placement` says and plays it from its
    /// start item, unless it was added while something else plays.
    pub(super) fn play_resolved(
        &mut self,
        resolved: &mut Resolved,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let media = std::mem::take(&mut resolved.media);
        let first_new = match placement {
            Placement::Replace => {
                self.queue.replace(media);
                self.skips.played();
                0
            }
            Placement::Enqueue => self.queue.append(media),
            Placement::PlayNext => self.queue.insert_next(media),
        };
        if placement == Placement::Replace || !self.is_playing(cx) {
            self.play(first_new + resolved.start, window, cx);
        }
        cx.notify();
    }

    /// Plays item `index` of the queue.
    pub(super) fn play(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(media) = self.queue.set_current(index).cloned() else {
            return;
        };
        let player = self.player(window, cx);
        player.update(cx, |player, cx| player.open(media, cx));
        // The video replaces the empty state and its focused Open button; with nothing focused
        // the player's keys would reach nothing.
        if self.open_button.is_focused(window) || window.focused(cx).is_none() {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// The current item ended: the next one plays.
    pub(super) fn advance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.queue.next(self.ends()) {
            self.play(next, window, cx);
        }
    }

    /// The current item couldn't play: skips to the next one, unless too many failed in a row.
    /// The last item's error stays on screen instead.
    pub(super) fn skip_failed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(next) = self.queue.next(self.ends()) else { return };
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
