//! Playback ▸ and More Settings ▸: speeds, chapters, jumps and the settings panel's tabs.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use super::support::{
    chapters, checked, choose, emit, enabled, find, labels, player, playing, reason, submenu,
};
use super::{items, pick};
use crate::engine::backend::BackendEvent;
use crate::engine::mock::MockMedia;
use crate::main_window::tests::main_view;

/// A seekable clip ten minutes long, with room for chapters a minute apart.
fn ten_minutes() -> MockMedia {
    MockMedia { duration: Some(Duration::from_secs(600)), ..MockMedia::default() }
}

fn position(cx: &mut VisualTestContext) -> Duration {
    let player = player(cx);
    player.read_with(cx, |player, _| player.position())
}

fn rate(cx: &mut VisualTestContext) -> f32 {
    let player = player(cx);
    player.read_with(cx, |player, _| player.rate().get())
}

#[gpui::test]
fn speed_checks_the_playing_speed_and_plays_the_one_chosen(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    let speeds = items(cx);
    let speeds = submenu(find(&speeds, &["Playback", "Speed"]));
    assert_eq!(labels(speeds)[..3], ["0.25×", "0.5×", "0.75×"]);
    assert_eq!(checked(speeds), ["1×"]);

    pick(cx, &["Playback", "Speed", "1.5×"]);
    assert!((rate(cx) - 1.5).abs() < 1e-3);
    assert_eq!(checked(submenu(find(&items(cx), &["Playback", "Speed"]))), ["1.5×"]);

    pick(cx, &["Playback", "Speed", "Normal Speed"]);
    assert!((rate(cx) - 1.0).abs() < 1e-3);
    pick(cx, &["Playback", "Speed", "Faster"]);
    assert!(rate(cx) > 1.0);
}

#[gpui::test]
fn a_live_stream_plays_at_normal_speed_and_cant_jump(cx: &mut TestAppContext) {
    let live = MockMedia { duration: None, seekable: false, ..MockMedia::default() };
    let (cx, _mock) = playing(cx, live);
    let items = items(cx);
    let speed = find(&items, &["Playback", "Speed"]);
    assert!(!enabled(speed));
    assert_eq!(reason(speed).as_deref(), Some("Live streams play at normal speed"));
    for jump in ["Jump Forward", "Jump Backward", "Jump to Time…", "Add Bookmark"] {
        let item = find(&items, &["Playback", jump]);
        assert!(!enabled(item), "{jump} is enabled");
        assert_eq!(reason(item).as_deref(), Some("This media can't seek"), "{jump}");
    }
}

#[gpui::test]
fn chapters_are_listed_with_the_playing_one_checked(cx: &mut TestAppContext) {
    let (cx, mock) = playing(cx, ten_minutes());
    assert!(!labels(submenu(find(&items(cx), &["Playback"]))).contains(&"Chapter".into()));
    emit(cx, &mock, chapters(&["Opening", "Middle", "Credits"]));
    let menu = items(cx);
    let chapter = submenu(find(&menu, &["Playback", "Chapter"]));
    let expected = ["Opening", "Middle", "Credits", "Next Chapter", "Previous Chapter"];
    assert_eq!(labels(chapter), expected);
    assert_eq!(checked(chapter), ["Opening"]);

    choose(cx, find(&menu, &["Playback", "Chapter", "Middle"]));
    assert_eq!(position(cx), Duration::from_secs(60));
    assert_eq!(checked(submenu(find(&items(cx), &["Playback", "Chapter"]))), ["Middle"]);
    pick(cx, &["Playback", "Chapter", "Next Chapter"]);
    assert_eq!(position(cx), Duration::from_secs(120));
}

#[gpui::test]
fn the_jumps_move_through_the_media(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    pick(cx, &["Playback", "Jump Forward"]);
    let ahead = position(cx);
    assert!(ahead > Duration::ZERO);
    pick(cx, &["Playback", "Jump Backward"]);
    assert!(position(cx) < ahead);
}

#[gpui::test]
fn jump_to_time_opens_the_time_for_typing(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    pick(cx, &["Playback", "Jump to Time…"]);
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, _| main.control_bar.is_editing_time()));
}

#[gpui::test]
fn more_settings_opens_the_settings_panel(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    let menu = items(cx);
    let tabs = ["Playback Settings…", "Video Settings…", "Audio Settings…", "Subtitle Settings…"];
    assert_eq!(labels(submenu(find(&menu, &["More Settings"]))), tabs);
    choose(cx, find(&menu, &["More Settings", "Audio Settings…"]));
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, _| main.settings_panel.is_open()));
}

#[gpui::test]
fn emitted_positions_move_the_checked_chapter(cx: &mut TestAppContext) {
    let (cx, mock) = playing(cx, ten_minutes());
    emit(cx, &mock, chapters(&["One", "Two"]));
    emit(cx, &mock, BackendEvent::Position(Duration::from_secs(90)));
    assert_eq!(checked(submenu(find(&items(cx), &["Playback", "Chapter"]))), ["Two"]);
}
