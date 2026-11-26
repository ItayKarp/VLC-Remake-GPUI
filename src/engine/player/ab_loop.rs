//! The [`Player`]'s A-B loop (S8.13.4): marks taken where playback is, and the seek back to A
//! once playback reaches B. A loop belongs to the media it was marked on.

use std::time::Duration;

use gpui::Context;

use super::Player;
use crate::engine::ab_loop::AbLoop;
use crate::engine::values::SeekTarget;

impl Player {
    /// The A-B loop's marks.
    pub fn ab_loop(&self) -> AbLoop {
        self.ab_loop
    }

    /// Marks A, then B, where playback is, then ends the loop ([`AbLoop::marked`]). Returns the
    /// loop now, or `None`, changing nothing, when the media can't loop: none is open, or it
    /// can't seek (a live stream).
    pub fn mark_ab_loop(&mut self, cx: &mut Context<Self>) -> Option<AbLoop> {
        if !self.state.is_active() || self.seeks.is_seekable() != Some(true) {
            return None;
        }
        self.ab_loop = self.ab_loop.marked(self.position.shown(), self.position.duration());
        cx.notify();
        Some(self.ab_loop)
    }

    /// Ends the A-B loop.
    pub fn clear_ab_loop(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.ab_loop) != AbLoop::Off {
            cx.notify();
        }
    }

    /// Playback reached `time`: goes back to A once it passed B. Not while recording, which
    /// can't seek (S8.6).
    pub(super) fn wrap_ab_loop(&mut self, time: Duration, cx: &mut Context<Self>) {
        if self.recording.is_some() {
            return;
        }
        if let Some(a) = self.ab_loop.wraps(time) {
            self.seek(SeekTarget::Absolute(a), cx);
        }
    }
}
