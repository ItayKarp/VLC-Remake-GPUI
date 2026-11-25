//! Video in a window of its own ("Play video inside main window" off): it opens once the mock's
//! media turns out to have video, takes fullscreen, closes a moment after playback stops, and
//! closing it stops playback, unless it was the desktop wallpaper, which comes back.

use gpui::{TestAppContext, VisualTestContext};

use super::*;
use crate::engine::backend::BackendEvent;
use crate::engine::media_info::VideoSize;
use crate::engine::state::PlaybackState;
use crate::main_window::now_playing::tests::{Playing, draw, open_playing};
use crate::main_window::tests::main_view;

const SIZE: VideoSize = VideoSize { width: 640, height: 360 };

/// The mock's clip playing with video in a window of its own.
fn separate(cx: &mut TestAppContext) -> (&mut VisualTestContext, Playing) {
    let (cx, playing) = open_playing(cx);
    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.interface.play_video_in_main_window = false);
    });
    playing.mock.emit(BackendEvent::VideoSize(Some(SIZE)));
    draw(cx);
    (cx, playing)
}

fn video_window(cx: &mut VisualTestContext) -> Option<WindowHandle<VideoWindow>> {
    main_view(cx).read_with(cx, |main, _| match &main.windowing.separate {
        SeparateVideo::Open { handle, .. } => Some(*handle),
        _ => None,
    })
}

fn state(cx: &mut VisualTestContext, playing: &Playing) -> PlaybackState {
    playing.player.read_with(cx, |player, _| player.state())
}

/// Closes the video's window as its close button does.
fn close(cx: &mut VisualTestContext, handle: WindowHandle<VideoWindow>) {
    cx.update(|_, cx| handle.update(cx, |_, window, _| window.remove_window()).ok());
    draw(cx);
}

#[gpui::test]
fn video_opens_in_its_own_window_and_takes_fullscreen(cx: &mut TestAppContext) {
    let (cx, _playing) = separate(cx);
    assert!(video_window(cx).is_some());
    let main = main_view(cx);
    assert!(main.read_with(cx, MainWindow::showing_video).is_none(), "not here");
    main.update_in(cx, MainWindow::toggle_fullscreen);
    draw(cx);
    assert!(main.update_in(cx, |main, window, _| main.video_is_fullscreen(window)));
    assert!(!cx.update(|window, _| window.is_fullscreen()), "the main window stays as it was");
    main.update_in(cx, |main, window, cx| main.fit_to_video((320., 180.), window, cx));
    draw(cx);
}

#[gpui::test]
fn the_window_closes_a_moment_after_playback_stops(cx: &mut TestAppContext) {
    let (cx, playing) = separate(cx);
    playing.player.update(cx, Player::stop);
    draw(cx);
    assert!(video_window(cx).is_some(), "the next video may come");
    cx.executor().advance_clock(CLOSE_DELAY);
    draw(cx);
    assert!(video_window(cx).is_none());
}

#[gpui::test]
fn closing_the_window_stops_playback(cx: &mut TestAppContext) {
    let (cx, playing) = separate(cx);
    let handle = video_window(cx).expect("a video window");
    close(cx, handle);
    assert!(video_window(cx).is_none());
    assert_eq!(state(cx, &playing), PlaybackState::Stopped);
}

#[gpui::test]
fn a_wallpaper_taken_away_comes_back(cx: &mut TestAppContext) {
    let (cx, playing) = separate(cx);
    let handle = video_window(cx).expect("a video window");
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.windowing.wallpaper = Some(MainWindow::keep_as_wallpaper(handle, window, cx));
    });
    cx.executor().advance_clock(Duration::from_secs(5));
    draw(cx);
    close(cx, handle);
    assert_eq!(state(cx, &playing), PlaybackState::Playing, "playback goes on");
    assert!(video_window(cx).is_some_and(|reopened| reopened != handle), "a new window opens");
    assert!(main.read_with(cx, |main, _| main.windowing.wallpaper.is_none()));
}

#[gpui::test]
fn start_fullscreen_goes_fullscreen_once_video_shows(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.video.start_fullscreen = true));
    playing.mock.emit(BackendEvent::VideoSize(Some(SIZE)));
    draw(cx);
    assert!(cx.update(|window, _| window.is_fullscreen()));
}

#[gpui::test]
fn only_windows_plays_video_as_wallpaper(cx: &mut TestAppContext) {
    let (cx, _playing) = open_playing(cx);
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.video.desktop_wallpaper = true));
    assert_eq!(cx.update(|_, cx| wallpaper::wanted(cx)), cfg!(target_os = "windows"));
}
