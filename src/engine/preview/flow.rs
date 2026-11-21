//! The preview decoder's request logic, apart from any decoder, so it can be tested.
//!
//! The decoder starts playing the media muted and pauses on its first picture. Each request
//! then seeks while paused, resumes, and pauses again on the next picture, which is the one at
//! the new time (libVLC drops the pictures before a precise seek's target).
//!
//! Two kinds of stale picture must not answer a request. One already on its way when the
//! decoder pauses still arrives, so the next seek waits until the decoder says it paused. And a
//! paused libVLC shows its last picture again every so often, until the seek lands; so a
//! picture only counts once the decoder reported a new time after the seek.
//!
//! Only one request runs at a time; while it runs, a newer request replaces any waiting one,
//! so a fast-moving pointer costs one decode per picture actually shown, never a backlog.

use std::time::{Duration, Instant};

use super::PreviewRequest;

/// How long the media may take to show its first picture before previews give up on it.
pub const START_TIMEOUT: Duration = Duration::from_secs(8);
/// How long one picture may take before its request is answered without one.
pub const SEEK_TIMEOUT: Duration = Duration::from_secs(3);
/// How long to wait for the decoder to say it paused before carrying on anyway.
pub const PAUSE_TIMEOUT: Duration = Duration::from_millis(500);

/// What the decoder should do next, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Seek the paused player to this time.
    Seek(Duration),
    /// Let it play until the next picture.
    Resume,
    /// Pause it.
    Pause,
    /// Answer this request with the picture just shown.
    Deliver(u64),
    /// Answer this request without a picture.
    Fail(u64),
    /// Start the media again from the top (it reached its end), staying muted.
    Restart,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// No media.
    Closed,
    /// Playing towards the first picture.
    Starting { since: Instant },
    /// Asked to pause, waiting for it to take effect.
    Pausing { since: Instant },
    /// Paused, waiting for a request.
    Ready,
    /// Working on `request`; `landed` once the decoder reported a time since the seek.
    Seeking { request: PreviewRequest, since: Instant, landed: bool },
    /// The media can't make pictures (an error, no video, or it never started).
    Broken,
}

/// The state of the decoder and its one waiting request.
#[derive(Debug)]
pub struct Flow {
    phase: Phase,
    waiting: Option<PreviewRequest>,
}

impl Default for Flow {
    fn default() -> Self {
        Self { phase: Phase::Closed, waiting: None }
    }
}

impl Flow {
    /// New media started playing; earlier requests are forgotten.
    pub fn opened(&mut self, now: Instant) {
        self.phase = Phase::Starting { since: now };
        self.waiting = None;
    }

    /// The media was closed.
    pub fn closed(&mut self) {
        *self = Self::default();
    }

    /// The UI asked for `request`.
    pub fn request(&mut self, request: PreviewRequest, now: Instant) -> Vec<Action> {
        match self.phase {
            Phase::Closed | Phase::Broken => vec![Action::Fail(request.id)],
            Phase::Ready => self.start(request, now),
            Phase::Starting { .. } | Phase::Pausing { .. } | Phase::Seeking { .. } => {
                self.waiting = Some(request);
                Vec::new()
            }
        }
    }

    /// The decoder showed a picture.
    pub fn picture(&mut self, now: Instant) -> Vec<Action> {
        let actions = match self.phase {
            Phase::Starting { .. } => vec![Action::Pause],
            Phase::Seeking { request, landed: true, .. } => {
                vec![Action::Pause, Action::Deliver(request.id)]
            }
            // Before the seek lands, a picture is the paused one shown again.
            Phase::Seeking { landed: false, .. }
            | Phase::Closed
            | Phase::Pausing { .. }
            | Phase::Ready
            | Phase::Broken => return Vec::new(),
        };
        self.phase = Phase::Pausing { since: now };
        actions
    }

    /// The decoder reported a new playback time: a running seek has landed.
    pub fn time_changed(&mut self) {
        if let Phase::Seeking { landed, .. } = &mut self.phase {
            *landed = true;
        }
    }

    /// The decoder paused: pictures on their way have arrived, so the next seek may start.
    pub fn paused(&mut self, now: Instant) -> Vec<Action> {
        if !matches!(self.phase, Phase::Pausing { .. }) {
            return Vec::new();
        }
        self.phase = Phase::Ready;
        self.next(now)
    }

    /// Playback reached the end of the media (a request near the end ran past it).
    pub fn ended(&mut self, now: Instant) -> Vec<Action> {
        let actions = match self.phase {
            Phase::Seeking { request, .. } => vec![Action::Fail(request.id), Action::Restart],
            Phase::Pausing { .. } | Phase::Ready => vec![Action::Restart],
            Phase::Starting { .. } => return self.broken(),
            Phase::Closed | Phase::Broken => return Vec::new(),
        };
        // The waiting request runs once the restarted media shows its first picture.
        self.phase = Phase::Starting { since: now };
        actions
    }

    /// The decoder failed on this media.
    pub fn failed(&mut self) -> Vec<Action> {
        if self.phase == Phase::Closed {
            return Vec::new();
        }
        self.broken()
    }

    /// Time passed: gives up on a start or a picture that took too long.
    pub fn tick(&mut self, now: Instant) -> Vec<Action> {
        match self.phase {
            Phase::Starting { since } if now.duration_since(since) >= START_TIMEOUT => {
                self.broken()
            }
            Phase::Seeking { request, since, .. } if now.duration_since(since) >= SEEK_TIMEOUT => {
                self.phase = Phase::Pausing { since: now };
                vec![Action::Pause, Action::Fail(request.id)]
            }
            Phase::Pausing { since } if now.duration_since(since) >= PAUSE_TIMEOUT => {
                self.paused(now)
            }
            _ => Vec::new(),
        }
    }

    /// When [`Self::tick`] next has something to do, if ever.
    pub fn deadline(&self) -> Option<Instant> {
        match self.phase {
            Phase::Starting { since } => Some(since + START_TIMEOUT),
            Phase::Seeking { since, .. } => Some(since + SEEK_TIMEOUT),
            Phase::Pausing { since } => Some(since + PAUSE_TIMEOUT),
            Phase::Closed | Phase::Ready | Phase::Broken => None,
        }
    }

    fn start(&mut self, request: PreviewRequest, now: Instant) -> Vec<Action> {
        self.phase = Phase::Seeking { request, since: now, landed: false };
        vec![Action::Seek(request.time), Action::Resume]
    }

    fn next(&mut self, now: Instant) -> Vec<Action> {
        self.waiting.take().map_or_else(Vec::new, |request| self.start(request, now))
    }

    fn broken(&mut self) -> Vec<Action> {
        let running = match self.phase {
            Phase::Seeking { request, .. } => Some(request.id),
            _ => None,
        };
        self.phase = Phase::Broken;
        let waiting = self.waiting.take().map(|request| request.id);
        running.into_iter().chain(waiting).map(Action::Fail).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::Action::{Deliver, Fail, Pause, Restart, Resume, Seek};
    use super::*;

    fn request(id: u64, secs: u64) -> PreviewRequest {
        PreviewRequest { id, time: Duration::from_secs(secs) }
    }

    fn ready(now: Instant) -> Flow {
        let mut flow = Flow::default();
        flow.opened(now);
        assert_eq!(flow.picture(now), [Pause]);
        assert_eq!(flow.paused(now), []);
        flow
    }

    fn secs(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn a_request_seeks_resumes_and_delivers_the_next_picture() {
        let now = Instant::now();
        let mut flow = ready(now);
        assert_eq!(flow.request(request(1, 30), now), [Seek(secs(30)), Resume]);
        assert_eq!(flow.picture(now), [], "the paused picture shown again is stale");
        flow.time_changed();
        assert_eq!(flow.picture(now), [Pause, Deliver(1)]);
        assert_eq!(flow.picture(now), [], "a picture on its way while pausing is ignored");
    }

    #[test]
    fn requests_during_a_seek_keep_only_the_newest_and_wait_for_the_pause() {
        let now = Instant::now();
        let mut flow = ready(now);
        flow.request(request(1, 10), now);
        assert_eq!(flow.request(request(2, 20), now), []);
        assert_eq!(flow.request(request(3, 30), now), []);
        flow.time_changed();
        assert_eq!(flow.picture(now), [Pause, Deliver(1)]);
        assert_eq!(flow.picture(now), [], "the stale picture must not answer request 3");
        assert_eq!(flow.paused(now), [Seek(secs(30)), Resume]);
    }

    #[test]
    fn a_request_before_the_first_picture_waits_for_it() {
        let now = Instant::now();
        let mut flow = Flow::default();
        flow.opened(now);
        assert_eq!(flow.request(request(1, 5), now), []);
        assert_eq!(flow.picture(now), [Pause]);
        assert_eq!(flow.paused(now), [Seek(secs(5)), Resume]);
    }

    #[test]
    fn a_pause_that_is_never_confirmed_carries_on_after_a_while() {
        let now = Instant::now();
        let mut flow = Flow::default();
        flow.opened(now);
        flow.request(request(1, 5), now);
        flow.picture(now);
        assert_eq!(flow.deadline(), Some(now + PAUSE_TIMEOUT));
        assert_eq!(flow.tick(now + PAUSE_TIMEOUT), [Seek(secs(5)), Resume]);
    }

    #[test]
    fn a_slow_picture_times_out_and_the_next_request_runs() {
        let now = Instant::now();
        let mut flow = ready(now);
        flow.request(request(1, 10), now);
        flow.request(request(2, 20), now);
        assert_eq!(flow.tick(now + SEEK_TIMEOUT / 2), []);
        let later = now + SEEK_TIMEOUT;
        assert_eq!(flow.tick(later), [Pause, Fail(1)]);
        assert_eq!(flow.paused(later), [Seek(secs(20)), Resume]);
        assert_eq!(flow.deadline(), Some(later + SEEK_TIMEOUT));
    }

    #[test]
    fn media_that_never_shows_a_picture_fails_every_request() {
        let now = Instant::now();
        let mut flow = Flow::default();
        flow.opened(now);
        flow.request(request(1, 10), now);
        assert_eq!(flow.tick(now + START_TIMEOUT), [Fail(1)]);
        assert_eq!(flow.request(request(2, 20), now), [Fail(2)]);
        assert_eq!(flow.deadline(), None);
    }

    #[test]
    fn running_past_the_end_restarts_and_keeps_the_waiting_request() {
        let now = Instant::now();
        let mut flow = ready(now);
        flow.request(request(1, 99), now);
        flow.request(request(2, 50), now);
        assert_eq!(flow.ended(now), [Fail(1), Restart]);
        assert_eq!(flow.picture(now), [Pause]);
        assert_eq!(flow.paused(now), [Seek(secs(50)), Resume]);
    }

    #[test]
    fn an_error_fails_what_runs_and_waits() {
        let now = Instant::now();
        let mut flow = ready(now);
        flow.request(request(1, 10), now);
        flow.request(request(2, 20), now);
        assert_eq!(flow.failed(), [Fail(1), Fail(2)]);
    }

    #[test]
    fn opening_new_media_forgets_the_waiting_request() {
        let now = Instant::now();
        let mut flow = Flow::default();
        flow.opened(now);
        flow.request(request(1, 10), now);
        flow.opened(now);
        assert_eq!(flow.picture(now), [Pause]);
        flow.closed();
        assert_eq!(flow.request(request(2, 1), now), [Fail(2)]);
    }
}
