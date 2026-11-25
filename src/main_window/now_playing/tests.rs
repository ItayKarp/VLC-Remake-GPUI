//! The main window's player on the mock backend: what the status line says as playback goes,
//! and helpers that let other main window tests play something without libVLC.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::engine::backend::{Backend, BackendEvent, Command};
use crate::engine::language::Language;
use crate::engine::mock::{MockBackend, MockHandle, MockMedia};
use crate::engine::player::Player;
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::engine::tracks::{Track, TrackId, TrackKind, Tracks};
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open};
use crate::ui::toast::Level;
use crate::video::Output;

/// What a test plays: the window's player and the mock behind it.
pub(in crate::main_window) struct Playing {
    /// The window's player.
    pub player: Entity<Player>,
    /// The mock backend's handle, to send events and read commands.
    pub mock: MockHandle,
}

/// Gives the window a player on a mock backend pretending to play `media`, and plays
/// `mock://clip` from the queue, as opening it does.
pub(in crate::main_window) fn play_mock(cx: &mut VisualTestContext, media: MockMedia) -> Playing {
    play_media(cx, media, Media::Url("mock://clip".into()))
}

/// Gives the window a player on a mock backend pretending to play `mock`, and plays `media`
/// from the queue.
pub(in crate::main_window) fn play_media(
    cx: &mut VisualTestContext,
    mock: MockMedia,
    media: Media,
) -> Playing {
    let slot = Rc::new(RefCell::new(None));
    let out = slot.clone();
    let main = main_view(cx);
    let player = main.update_in(cx, |main, window, cx| {
        let make = move |events| {
            let (backend, handle) = MockBackend::new(events, mock);
            *out.borrow_mut() = Some(handle);
            Box::new(backend) as Box<dyn Backend>
        };
        let player = main.attach_player(Output::new(), make, window, cx);
        main.queue.replace(vec![media]);
        main.play(0, window, cx);
        player
    });
    cx.run_until_parked();
    let mock = slot.borrow_mut().take().expect("the player builds its backend at once");
    Playing { player, mock }
}

/// The mock's clip with a video, an English audio track of `channels` and English subtitles,
/// each selected.
pub(in crate::main_window) fn with_tracks(channels: u32) -> MockMedia {
    let track = |id, kind| Track {
        title: Some(format!("Track {id}")),
        language: Language::parse("eng"),
        ..Track::new(TrackId(id), kind)
    };
    let audio = Track { channels: Some(channels), ..track(2, TrackKind::Audio) };
    let tracks = Tracks {
        all: vec![track(1, TrackKind::Video), audio, track(3, TrackKind::Subtitle)],
        video: Some(TrackId(1)),
        audio: Some(TrackId(2)),
        subtitle: Some(TrackId(3)),
    };
    MockMedia { tracks, ..MockMedia::default() }
}

/// The main window playing the mock's one-minute clip.
pub(in crate::main_window) fn open_playing(
    cx: &mut TestAppContext,
) -> (&mut VisualTestContext, Playing) {
    let cx = open(cx);
    let playing = play_mock(cx, MockMedia::default());
    (cx, playing)
}

/// Paints a frame, as the window would, after letting every task settle.
pub(in crate::main_window) fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// The toasts showing: each level and message.
pub(in crate::main_window) fn toasts(cx: &mut VisualTestContext) -> Vec<(Level, String)> {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.toasts.read(cx).shown())
}

/// The last on-screen message.
pub(in crate::main_window) fn osd(cx: &mut VisualTestContext) -> String {
    main_view(cx).read_with(cx, |main, _| main.osd_text().to_owned())
}

/// The status line's text, and whether it offers Cancel.
fn status(cx: &mut VisualTestContext) -> Option<(String, bool)> {
    let main = main_view(cx);
    main.update(cx, |main, cx| main.status(cx).map(|(text, cancel)| (text.to_string(), cancel)))
}

#[gpui::test]
fn the_status_line_follows_playback(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    assert_eq!(status(cx), Some(("Playing mock://clip".into(), false)));
    playing.player.update(cx, Player::pause);
    cx.run_until_parked();
    assert_eq!(status(cx), Some(("Paused: mock://clip".into(), false)));
    playing.mock.emit(BackendEvent::State(PlaybackState::Buffering));
    cx.run_until_parked();
    assert_eq!(status(cx), Some(("Buffering mock://clip…".into(), false)));
    playing.player.update(cx, Player::stop);
    cx.run_until_parked();
    assert_eq!(status(cx), Some(("Stopped mock://clip".into(), false)));
}

#[gpui::test]
fn a_failed_open_says_why(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    playing.mock.emit(BackendEvent::State(PlaybackState::Error(ErrorKind::Corrupt)));
    cx.run_until_parked();
    let (text, cancel) = status(cx).expect("a status line");
    assert_eq!(text, "Couldn't play mock://clip: the file is damaged.");
    assert!(!cancel);
}

#[gpui::test]
fn opening_offers_cancel_which_stops_it(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, MockMedia::manual());
    playing.mock.emit(BackendEvent::State(PlaybackState::Opening));
    draw(cx);
    assert_eq!(status(cx), Some(("Opening mock://clip…".into(), true)));
    playing.mock.take_commands();
    main_view(cx).update(cx, MainWindow::cancel);
    assert!(playing.mock.take_commands().contains(&Command::Stop));
}

#[gpui::test]
fn nothing_open_says_nothing(cx: &mut TestAppContext) {
    let cx = open(cx);
    assert_eq!(status(cx), None);
    let main = main_view(cx);
    assert!(!main.update(cx, |main, cx| main.is_playing(cx)));
}
