//! How the [`Player`] handles backend events: dropping stale ones, moving the state machine and
//! sending the next coalesced command.

use std::time::{Duration, Instant};

use gpui::Context;

use super::{Intent, Player, PlayerEvent};
use crate::engine::backend::{BackendEvent, Tagged};
use crate::engine::state::PlaybackState;

impl Player {
    /// Handles one event from the backend. Events about an earlier open's media are dropped.
    pub(super) fn receive(&mut self, tagged: Tagged, cx: &mut Context<Self>) {
        if tagged.generation != self.generation && tagged.event.is_about_media() {
            tracing::trace!("engine: dropped a stale {:?}", tagged.event);
            return;
        }
        match tagged.event {
            BackendEvent::State(next) => self.transition(next, cx),
            BackendEvent::Buffering(percent) => {
                self.info.buffer_fill = percent;
                cx.emit(PlayerEvent::Buffering(percent));
            }
            BackendEvent::Position(time) => self.position_reported(time, cx),
            BackendEvent::Duration(duration) => self.duration_reported(duration, cx),
            BackendEvent::Seekable(seekable) => self.seekable_reported(seekable, cx),
            BackendEvent::Pausable(pausable) => {
                if self.info.pausable.replace(pausable) != Some(pausable) {
                    cx.emit(PlayerEvent::PausableChanged(pausable));
                }
            }
            BackendEvent::Tracks(tracks) => self.tracks_reported(tracks, cx),
            BackendEvent::VideoSize(size) => {
                if std::mem::replace(&mut self.info.video_size, size) != size {
                    cx.emit(PlayerEvent::VideoSizeChanged(size));
                }
            }
            BackendEvent::Chapters(chapters) => {
                self.info.chapters = chapters;
                cx.emit(PlayerEvent::ChaptersChanged);
            }
            BackendEvent::Metadata(metadata) => {
                self.info.metadata = metadata;
                cx.emit(PlayerEvent::MetadataChanged);
            }
            BackendEvent::SeekCompleted => self.send_next_seek(),
            BackendEvent::FrameStepped => self.send_next_step(),
            BackendEvent::AudioDevices(devices) => self.devices_reported(devices, cx),
            BackendEvent::Corked(corked) => self.corked(corked, cx),
        }
        cx.notify();
    }

    /// Moves the state machine to `next`, if the graph allows it; repeats and disallowed
    /// transitions are ignored.
    pub(super) fn transition(&mut self, next: PlaybackState, cx: &mut Context<Self>) {
        if next == self.state || !self.state.can_become(next) {
            tracing::debug!("engine: ignored {:?} -> {next:?}", self.state);
            return;
        }
        self.state = next;
        cx.emit(PlayerEvent::StateChanged(next));
        match next {
            PlaybackState::Playing if self.intent == Intent::Pause => {
                self.with_backend(|backend| backend.pause());
            }
            PlaybackState::Paused if self.intent == Intent::Play => {
                self.with_backend(|backend| backend.play());
            }
            PlaybackState::Ended => {
                self.cancel_queues();
                cx.emit(PlayerEvent::EndReached);
            }
            PlaybackState::Stopped => self.cancel_queues(),
            PlaybackState::Error(kind) => {
                self.cancel_queues();
                cx.emit(PlayerEvent::Error(kind));
            }
            _ => {}
        }
        cx.notify();
    }

    fn position_reported(&mut self, time: Duration, cx: &mut Context<Self>) {
        // Until a seek lands, the backend still reports the old position.
        if self.seeks.is_busy() {
            return;
        }
        if let Some(shown) = self.position.report(time, Instant::now()) {
            cx.emit(PlayerEvent::PositionChanged(shown));
        }
    }

    fn duration_reported(&mut self, duration: Option<Duration>, cx: &mut Context<Self>) {
        let before = self.position.duration();
        self.position.set_duration(duration);
        let after = self.position.duration();
        if after != before {
            cx.emit(PlayerEvent::DurationChanged(after));
        }
    }

    fn seekable_reported(&mut self, seekable: bool, cx: &mut Context<Self>) {
        if self.seeks.is_seekable() != Some(seekable) {
            cx.emit(PlayerEvent::SeekableChanged(seekable));
        }
        if let Some(seek) = self.seeks.set_seekable(seekable) {
            self.with_backend(|backend| backend.seek(seek));
        }
    }

    fn send_next_seek(&mut self) {
        if let Some(seek) = self.seeks.completed() {
            self.with_backend(|backend| backend.seek(seek));
        }
    }

    fn send_next_step(&mut self) {
        if let Some(direction) = self.steps.completed() {
            self.with_backend(|backend| backend.frame_step(direction));
        }
    }

    fn cancel_queues(&mut self) {
        self.seeks.cancel();
        self.steps.reset();
    }
}
