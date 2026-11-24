//! The [`Player`]'s streaming out (S8.12): the stream the media open now goes to. A stream
//! belongs to the media it was started on: opening other media ends it, unless that media opens
//! streaming ([`Start::stream`](crate::engine::values::Start)). Playing on a cast device and
//! streaming don't mix, as both take libVLC's stream output, so starting one stops the other.

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::engine::stream_out::StreamTarget;

impl Player {
    /// The stream the media open now goes to; `None` while it doesn't stream.
    pub fn stream_target(&self) -> Option<&StreamTarget> {
        self.stream.as_ref()
    }

    /// Streams the media open now as `target` says, from where it is, or stops streaming it with
    /// `None`. Starting needs media open, and stops casting.
    pub fn stream(&mut self, target: Option<StreamTarget>, cx: &mut Context<Self>) {
        if self.stream == target || (target.is_some() && self.media.is_none()) {
            return;
        }
        if target.is_some() {
            self.cast_to(None, cx);
        }
        self.stream = target;
        self.with_backend(|backend| backend.stream(target));
        cx.emit(PlayerEvent::StreamChanged);
        cx.notify();
    }

    /// Keeps the stream the media opening now starts with, saying when that ends another.
    pub(super) fn stream_opened(&mut self, target: Option<StreamTarget>, cx: &mut Context<Self>) {
        if self.stream != target {
            self.stream = target;
            cx.emit(PlayerEvent::StreamChanged);
        }
    }

    /// Ends a stream that couldn't start; the media plays on without it.
    pub(super) fn stream_failed(&mut self, cx: &mut Context<Self>) {
        if self.stream.take().is_some() {
            cx.emit(PlayerEvent::StreamChanged);
            cx.emit(PlayerEvent::StreamFailed);
        }
    }

    /// Stops streaming before casting starts.
    pub(super) fn stop_streaming_for_cast(&mut self, cx: &mut Context<Self>) {
        if self.stream.is_some() {
            self.stream(None, cx);
        }
    }
}
