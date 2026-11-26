//! The [`Player`]'s recording (S8.6): starting and stopping it, and how long it has recorded.
//!
//! The time counts only while the media plays: a paused stream sends nothing to record.

use std::path::Path;
use std::time::{Duration, Instant};

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::engine::recording::Recording;
use crate::engine::state::PlaybackState;

/// How long a recording has run, counting only the time the media played.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RecordClock {
    /// The time counted before the media last started playing.
    banked: Duration,
    /// When the media last started playing; `None` while it isn't.
    since: Option<Instant>,
}

impl RecordClock {
    /// A clock starting now, running if the media is playing.
    fn started(playing: bool, now: Instant) -> Self {
        Self { banked: Duration::ZERO, since: playing.then_some(now) }
    }

    /// Runs the clock while the media plays and stops it while it doesn't.
    pub(super) fn follow(&mut self, playing: bool, now: Instant) {
        match (self.since, playing) {
            (None, true) => self.since = Some(now),
            (Some(since), false) => {
                self.banked += now.saturating_duration_since(since);
                self.since = None;
            }
            _ => {}
        }
    }

    /// The time counted by `now`.
    fn elapsed(&self, now: Instant) -> Duration {
        self.banked
            + self.since.map_or(Duration::ZERO, |since| now.saturating_duration_since(since))
    }
}

impl Player {
    /// Starts recording the media playing into `folder` (S8.6). The answer arrives as
    /// [`PlayerEvent::RecordingChanged`]: started, or refused.
    pub fn start_recording(&mut self, folder: &Path) {
        if self.recording.is_none() {
            self.with_backend(|backend| backend.record(Some(folder)));
        }
    }

    /// Stops recording; [`PlayerEvent::RecordingChanged`] says once the file is finished.
    pub fn stop_recording(&mut self) {
        if self.recording.is_some() {
            self.with_backend(|backend| backend.record(None));
        }
    }

    /// How long the running recording has recorded; `None` while not recording.
    pub fn recorded(&self) -> Option<Duration> {
        self.recording.map(|clock| clock.elapsed(Instant::now()))
    }

    /// Keeps what the backend said about recording, and passes it on.
    pub(super) fn recording_reported(&mut self, recording: Recording, cx: &mut Context<Self>) {
        let playing = self.state == PlaybackState::Playing;
        self.recording = match recording {
            Recording::Started => Some(RecordClock::started(playing, Instant::now())),
            Recording::Stopped | Recording::Refused => None,
        };
        cx.emit(PlayerEvent::RecordingChanged(recording));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_counts_only_while_playing() {
        let start = Instant::now();
        let at = |seconds| start + Duration::from_secs(seconds);
        let mut clock = RecordClock::started(true, start);
        assert_eq!(clock.elapsed(at(3)), Duration::from_secs(3));
        clock.follow(false, at(3));
        assert_eq!(clock.elapsed(at(10)), Duration::from_secs(3));
        clock.follow(true, at(10));
        clock.follow(true, at(11));
        assert_eq!(clock.elapsed(at(12)), Duration::from_secs(5));
    }

    #[test]
    fn a_clock_started_paused_waits_for_playback() {
        let start = Instant::now();
        let mut clock = RecordClock::started(false, start);
        assert_eq!(clock.elapsed(start + Duration::from_secs(4)), Duration::ZERO);
        clock.follow(true, start + Duration::from_secs(4));
        assert_eq!(clock.elapsed(start + Duration::from_secs(6)), Duration::from_secs(2));
    }
}
