//! The miniplayer card in full library mode over the mock's media: what it shows, and its
//! progress bar seeking where it is clicked.

use std::time::Duration;

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext, point};

use super::*;
use crate::engine::backend::BackendEvent;
use crate::engine::media_info::Metadata;
use crate::engine::mock::MockMedia;
use crate::engine::state::PlaybackState;
use crate::main_window::now_playing::tests::{Playing, draw, play_media};
use crate::main_window::tests::{main_view, open, tap};
use crate::opening::scratch::scratch_dir;

/// The window playing a song in a folder, its artist tagged, in full library mode.
fn card(cx: &mut TestAppContext) -> (&mut VisualTestContext, Playing) {
    let cx = open(cx);
    let album = scratch_dir().join("Album");
    std::fs::create_dir_all(&album).expect("a scratch folder");
    let path = album.join("song.mp3");
    std::fs::write(&path, b"audio").expect("a scratch file");
    let song = Media::File(path);
    let playing = play_media(cx, MockMedia::default(), song);
    let metadata = Metadata { artist: Some("The Band".into()), ..Metadata::default() };
    playing.mock.emit(BackendEvent::Metadata(metadata));
    main_view(cx).update_in(cx, MainWindow::open_full_library);
    draw(cx);
    (cx, playing)
}

#[test]
fn progress_is_a_fraction_of_a_known_length() {
    let secs = Duration::from_secs;
    assert!((progress(secs(30), Some(secs(120))) - 0.25).abs() < f32::EPSILON);
    assert!((progress(secs(200), Some(secs(120))) - 1.).abs() < f32::EPSILON);
    assert!(progress(secs(30), None).abs() < f32::EPSILON);
    assert!(progress(secs(30), Some(Duration::ZERO)).abs() < f32::EPSILON);
}

#[gpui::test]
fn the_card_shows_what_plays_and_seeks_where_clicked(cx: &mut TestAppContext) {
    let (cx, playing) = card(cx);
    let main = main_view(cx);
    let bounds = main.read_with(cx, |main, _| main.miniplayer.progress.bounds());
    let bounds = bounds.expect("the card's progress bar is drawn");
    let at = point(bounds.left() + bounds.size.width * 0.75, bounds.center().y);
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
    let position = playing.player.read_with(cx, |player, _| player.position());
    assert!(
        position > Duration::from_secs(40) && position < Duration::from_secs(50),
        "{position:?}"
    );
}

#[gpui::test]
fn the_card_stays_once_the_media_ended(cx: &mut TestAppContext) {
    let (cx, playing) = card(cx);
    playing.mock.emit(BackendEvent::State(PlaybackState::Ended));
    draw(cx);
    let main = main_view(cx);
    assert!(main.read_with(cx, MainWindow::miniplayer_player).is_some());
    main.update_in(cx, MainWindow::toggle_miniplayer_queue);
    draw(cx);
    assert!(main.read_with(cx, |main, _| main.is_miniplayer_queue_open()));
}

/// Tabs from the hover controls' holder `steps` stops on and presses Space there, as a
/// keyboard user reaches the card's buttons.
fn press_tab_stop(cx: &mut VisualTestContext, steps: usize) {
    let main = main_view(cx);
    let holder = main.read_with(cx, |main, _| main.miniplayer.overlay_focus.clone());
    let holder = holder.expect("the card is drawn");
    cx.update(|window, cx| holder.focus(window, cx));
    draw(cx);
    for _ in 0..steps {
        cx.update(gpui::Window::focus_next);
        draw(cx);
    }
    tap(cx, "space");
    draw(cx);
}

fn library_route(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| crate::settings::get(cx).local.library_routes.full.clone())
}

#[gpui::test]
fn close_stops_what_plays(cx: &mut TestAppContext) {
    let (cx, playing) = card(cx);
    press_tab_stop(cx, 1);
    assert_eq!(playing.player.read_with(cx, |player, _| player.state()), PlaybackState::Stopped);
}

#[gpui::test]
fn pop_out_goes_back_to_the_player(cx: &mut TestAppContext) {
    let (cx, playing) = card(cx);
    press_tab_stop(cx, 2);
    assert!(!main_view(cx).read_with(cx, |main, _| main.library_panel.is_full()));
    assert_eq!(playing.player.read_with(cx, |player, _| player.state()), PlaybackState::Playing);
}

#[gpui::test]
fn play_pause_pauses(cx: &mut TestAppContext) {
    let (cx, playing) = card(cx);
    press_tab_stop(cx, 3);
    assert_eq!(playing.player.read_with(cx, |player, _| player.state()), PlaybackState::Paused);
}

#[gpui::test]
fn the_chips_go_to_the_folder_and_the_artist(cx: &mut TestAppContext) {
    let (cx, _playing) = card(cx);
    press_tab_stop(cx, 4);
    assert!(library_route(cx).ends_with("Album"), "{}", library_route(cx));
    press_tab_stop(cx, 5);
    assert_eq!(library_route(cx), "music/artists:The Band");
    press_tab_stop(cx, 6);
    assert!(main_view(cx).read_with(cx, |main, _| main.is_miniplayer_queue_open()));
}
