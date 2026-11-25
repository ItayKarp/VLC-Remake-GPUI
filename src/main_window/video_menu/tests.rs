//! Tests of the video's right-click menu on a window playing on the mock backend: what each
//! submenu offers, what it checks and disables, and what choosing an entry does.

mod media;
mod playback;
pub(in crate::main_window) mod support;

use gpui::{TestAppContext, VisualTestContext};

use self::support::{choose, enabled, find, labels, player, playing, reason};
use crate::engine::mock::MockMedia;
use crate::main_window::tests::{main_view, open, tap};
use crate::ui::menu::MenuItem;

/// The video menu as it would open now.
fn items(cx: &mut VisualTestContext) -> Vec<MenuItem> {
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.video_menu_items(window, cx))
}

/// Chooses the video menu's entry at `path`, as it would open now.
fn pick(cx: &mut VisualTestContext, path: &[&str]) {
    let items = items(cx);
    choose(cx, find(&items, path));
}

/// Paints a frame, as the window would.
fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

#[gpui::test]
fn with_nothing_playing_the_media_entries_say_why_not(cx: &mut TestAppContext) {
    let cx = open(cx);
    let items = items(cx);
    assert!(enabled(find(&items, &["Play"])));
    for idle in ["Stop", "Playback", "Audio", "Subtitle", "More Settings"] {
        let item = find(&items, &[idle]);
        assert!(!enabled(item), "{idle} is enabled");
        assert_eq!(reason(item).as_deref(), Some("No media playing"), "{idle}");
    }
    assert!(!enabled(find(&items, &["Video", "Picture in Picture"])));
    assert!(!enabled(find(&items, &["Playlist", "Clear Queue"])));
}

#[gpui::test]
fn while_playing_the_transport_pauses_and_stops(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    let items = items(cx);
    let order = ["Pause", "Stop", "Previous Item", "Next Item", "Record", "Hide UI"];
    assert_eq!(labels(&items)[..order.len()], order);
    assert!(enabled(find(&items, &["Stop"])));
    choose(cx, find(&items, &["Pause"]));
    let player = player(cx);
    assert!(!player.read_with(cx, |player, _| player.wants_to_play()));
    assert_eq!(labels(&self::items(cx))[0], "Play");

    pick(cx, &["Stop"]);
    assert!(!player.read_with(cx, |player, _| player.state().is_active()));
}

#[gpui::test]
fn hide_ui_turns_into_show_ui(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    pick(cx, &["Hide UI"]);
    let items = items(cx);
    assert!(labels(&items).contains(&"Show UI".to_owned()));
    let jump = find(&items, &["Playback", "Jump to Time…"]);
    assert!(!enabled(jump));
    assert_eq!(reason(jump).as_deref(), Some("Leave Hide UI to use this"));
    assert_eq!(
        reason(find(&items, &["More Settings"])).as_deref(),
        Some("Leave Hide UI to use this")
    );
}

#[gpui::test]
fn the_playlist_shows_and_clears_the_queue(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    pick(cx, &["Playlist", "Show Queue"]);
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, _| main.queue_panel.is_open()));
    assert!(
        labels(support::submenu(find(&items(cx), &["Playlist"]))).contains(&"Hide Queue".into())
    );

    pick(cx, &["Playlist", "Clear Queue"]);
    assert_eq!(main.read_with(cx, |main, _| main.queue.len()), 0);
}

#[gpui::test]
fn add_to_queue_and_play_next_ask_for_files(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    let folder = crate::opening::scratch::scratch_dir();
    let (later, next) = (folder.join("Later.mkv"), folder.join("Next.mkv"));
    for file in [&later, &next] {
        std::fs::write(file, "video").unwrap();
    }
    pick(cx, &["Open Media", "Add to Queue…"]);
    cx.simulate_path_prompt_response(|_| Some(vec![later.clone()]));
    cx.run_until_parked();
    pick(cx, &["Open Media", "Play Next…"]);
    cx.simulate_path_prompt_response(|_| Some(vec![next.clone()]));
    cx.run_until_parked();
    let main = main_view(cx);
    let queued: Vec<String> = main.read_with(cx, |main, _| {
        main.queue.entries().iter().map(|entry| entry.media.display_name()).collect()
    });
    assert_eq!(queued, ["Clip.mkv", "Next.mkv", "Later.mkv"]);
}

#[gpui::test]
fn shift_f10_opens_the_menu_on_the_video_and_escape_closes_it(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    draw(cx);
    tap(cx, "shift-f10");
    draw(cx);
    let main = main_view(cx);
    let is_open = |cx: &mut VisualTestContext| {
        main.read_with(cx, |main, _| main.auto_hide.video_menu.is_some())
    };
    assert!(is_open(cx));
    tap(cx, "escape");
    assert!(!is_open(cx));
}

#[gpui::test]
fn shift_f10_does_nothing_without_video(cx: &mut TestAppContext) {
    let cx = open(cx);
    tap(cx, "shift-f10");
    let main = main_view(cx);
    assert!(main.read_with(cx, |main, _| main.auto_hide.video_menu.is_none()));
}
