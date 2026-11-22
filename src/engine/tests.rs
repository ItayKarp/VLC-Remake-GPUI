//! Player tests against the mock backend: the state machine as the UI sees it, and the S1.2
//! edge cases.

mod audio;
mod drawn;
mod sync;
mod tracks;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{AppContext as _, Entity, TestAppContext};

use super::backend::{BackendEvent, Command};
use super::mock::{MockBackend, MockHandle, MockMedia};
use super::player::{Player, PlayerEvent};
use super::state::{ErrorKind, PlaybackState};
use super::values::{BackendSeek, Offset, SeekTarget, Start, StepDirection};
use crate::app::media::Media;

/// A player on a mock backend, the handle driving the mock, and every event the player sent.
struct Rig {
    player: Entity<Player>,
    mock: MockHandle,
    events: Rc<RefCell<Vec<PlayerEvent>>>,
}

impl Rig {
    fn new(cx: &mut TestAppContext, media: MockMedia) -> Self {
        let slot = Rc::new(RefCell::new(None));
        let out = slot.clone();
        let player = cx.new(|cx| {
            Player::new(
                move |events| {
                    let (mock, handle) = MockBackend::new(events, media);
                    *out.borrow_mut() = Some(handle);
                    Box::new(mock)
                },
                cx,
            )
        });
        let events: Rc<RefCell<Vec<PlayerEvent>>> = Rc::default();
        let log = events.clone();
        cx.update(|cx| {
            cx.subscribe(&player, move |_, event: &PlayerEvent, _| {
                log.borrow_mut().push(event.clone());
            })
            .detach();
        });
        let mock = slot.borrow_mut().take().unwrap();
        Self { player, mock, events }
    }

    /// Opens a mock URL (no preflight) and lets every event arrive.
    fn open(&self, cx: &mut TestAppContext) {
        self.run(cx, |player, cx| player.open(Media::Url("mock://clip".into()), cx));
    }

    fn run(
        &self,
        cx: &mut TestAppContext,
        command: impl FnOnce(&mut Player, &mut gpui::Context<Player>),
    ) {
        self.player.update(cx, command);
        cx.run_until_parked();
    }

    fn emit(&self, cx: &mut TestAppContext, event: BackendEvent) {
        self.mock.emit(event);
        cx.run_until_parked();
    }

    fn state(&self, cx: &mut TestAppContext) -> PlaybackState {
        self.player.read_with(cx, |player, _| player.state())
    }

    fn states(&self) -> Vec<PlaybackState> {
        let events = self.events.borrow();
        events
            .iter()
            .filter_map(|event| match event {
                PlayerEvent::StateChanged(state) => Some(*state),
                _ => None,
            })
            .collect()
    }

    fn count(&self, wanted: &PlayerEvent) -> usize {
        self.events.borrow().iter().filter(|event| *event == wanted).count()
    }
}

fn secs(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

#[gpui::test]
fn opening_reaches_playing_through_buffering(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    let expected = [PlaybackState::Opening, PlaybackState::Buffering, PlaybackState::Playing];
    assert_eq!(rig.states(), expected);
    assert_eq!(rig.count(&PlayerEvent::DurationChanged(Some(secs(60)))), 1);
    assert_eq!(rig.count(&PlayerEvent::SeekableChanged(true)), 1);
}

#[gpui::test]
fn late_events_from_a_cancelled_open_are_dropped(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    let first = rig.mock.generation();
    rig.open(cx);
    rig.mock.emit_for(first, BackendEvent::State(PlaybackState::Playing));
    rig.mock.emit_for(first, BackendEvent::Duration(Some(secs(9))));
    cx.run_until_parked();
    assert_eq!(rig.state(cx), PlaybackState::Opening);
    assert_eq!(rig.player.read_with(cx, |player, _| player.duration()), None);
}

#[gpui::test]
fn rapid_seeks_send_only_the_first_and_the_latest(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.emit(cx, BackendEvent::Seekable(true));
    rig.mock.take_commands();
    for target in [10, 20, 30] {
        rig.run(cx, |player, cx| player.seek(SeekTarget::Absolute(secs(target)), cx));
    }
    rig.emit(cx, BackendEvent::SeekCompleted);
    let seeks =
        [Command::Seek(BackendSeek::Time(secs(10))), Command::Seek(BackendSeek::Time(secs(30)))];
    assert_eq!(rig.mock.take_commands(), seeks);
    assert_eq!(rig.player.read_with(cx, |player, _| player.position()), secs(30));
}

#[gpui::test]
fn a_seek_before_seekable_waits_and_is_dropped_on_live_media(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.mock.take_commands();
    rig.run(cx, |player, cx| player.seek(SeekTarget::Relative(Offset::from_millis(5_000)), cx));
    assert_eq!(rig.mock.take_commands(), []);
    rig.emit(cx, BackendEvent::Seekable(true));
    assert_eq!(rig.mock.take_commands(), [Command::Seek(BackendSeek::Time(secs(5)))]);

    rig.open(cx);
    rig.mock.take_commands();
    rig.run(cx, |player, cx| player.seek(SeekTarget::Fraction(0.5), cx));
    rig.emit(cx, BackendEvent::Seekable(false));
    assert_eq!(rig.mock.take_commands(), []);
}

#[gpui::test]
fn seeking_to_the_end_ends_once(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, |player, cx| player.seek(SeekTarget::Fraction(1.0), cx));
    rig.emit(cx, BackendEvent::State(PlaybackState::Ended));
    assert_eq!(rig.state(cx), PlaybackState::Ended);
    assert_eq!(rig.count(&PlayerEvent::EndReached), 1);
    assert_eq!(rig.count(&PlayerEvent::StateChanged(PlaybackState::Ended)), 1);
}

#[gpui::test]
fn a_pause_while_buffering_is_honored_when_buffering_ends(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.emit(cx, BackendEvent::State(PlaybackState::Buffering));
    rig.run(cx, Player::pause);
    rig.mock.take_commands();
    rig.emit(cx, BackendEvent::State(PlaybackState::Playing));
    assert_eq!(rig.mock.take_commands(), [Command::Pause]);
}

#[gpui::test]
fn toggle_pauses_and_resumes(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, Player::toggle);
    assert_eq!(rig.state(cx), PlaybackState::Paused);
    rig.run(cx, Player::toggle);
    assert_eq!(rig.state(cx), PlaybackState::Playing);
}

#[gpui::test]
fn play_after_stop_reopens_the_media(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, Player::stop);
    assert_eq!(rig.state(cx), PlaybackState::Stopped);
    rig.mock.take_commands();
    rig.run(cx, Player::play);
    let reopen = Command::Open {
        media: Media::Url("mock://clip".into()),
        start: Start::default(),
        generation: rig.mock.generation(),
    };
    assert_eq!(rig.mock.take_commands(), [reopen]);
    assert_eq!(rig.state(cx), PlaybackState::Playing);
}

#[gpui::test]
fn frame_steps_queue_behind_the_one_in_flight(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    rig.open(cx);
    rig.mock.take_commands();
    for _ in 0..3 {
        rig.run(cx, |player, cx| player.frame_step(StepDirection::Forward, cx));
    }
    assert_eq!(rig.mock.take_commands(), [Command::FrameStep(StepDirection::Forward)]);
    rig.emit(cx, BackendEvent::FrameStepped);
    rig.emit(cx, BackendEvent::FrameStepped);
    rig.emit(cx, BackendEvent::FrameStepped);
    assert_eq!(rig.mock.take_commands().len(), 2);
}

#[gpui::test]
fn a_missing_file_fails_before_reaching_the_backend(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    let missing = std::env::temp_dir().join("rvlc-engine-test-missing.mkv");
    rig.run(cx, |player, cx| player.open(Media::File(missing), cx));
    assert_eq!(rig.state(cx), PlaybackState::Error(ErrorKind::NotFound));
    assert_eq!(rig.count(&PlayerEvent::Error(ErrorKind::NotFound)), 1);
    assert!(
        !rig.mock.take_commands().iter().any(|command| matches!(command, Command::Open { .. }))
    );
}

#[gpui::test]
fn positions_are_clamped_and_shut_down_ignores_later_commands(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.emit(cx, BackendEvent::Position(secs(90)));
    assert_eq!(rig.player.read_with(cx, |player, _| player.position()), secs(60));
    let shutdown = rig.player.update(cx, |player, _| player.shut_down());
    assert_eq!(shutdown, Some(super::shutdown::Shutdown::Clean));
    rig.mock.take_commands();
    rig.run(cx, Player::pause);
    assert_eq!(rig.mock.take_commands(), []);
}

#[gpui::test]
fn open_at_holds_the_media_paused_where_asked_until_play(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    let start = Start { at: Duration::from_secs(42), paused: true };
    rig.run(cx, |player, cx| player.open_at(Media::Url("mock://clip".into()), start, cx));
    let opened = rig.mock.take_commands();
    assert!(matches!(opened.as_slice(), [Command::Open { start: s, .. }] if *s == start));
    assert_eq!(rig.state(cx), PlaybackState::Paused);
    rig.player.read_with(cx, |player, _| {
        assert!(!player.wants_to_play());
        assert_eq!(player.position(), Duration::from_secs(42));
    });
    rig.run(cx, Player::play);
    assert_eq!(rig.mock.take_commands(), [Command::Play]);
    assert_eq!(rig.state(cx), PlaybackState::Playing);
}
