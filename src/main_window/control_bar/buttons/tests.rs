//! What each icon button a bar can show looks like and does, with nothing open and with the
//! mock's clip playing.

use gpui::{TestAppContext, VisualTestContext};

use super::*;
use crate::main_window::now_playing::tests::open_playing;
use crate::main_window::tests::{main_view, open};

/// Every item that is an icon button.
const BUTTONS: [Item; 27] = [
    Item::Previous,
    Item::Next,
    Item::List,
    Item::Stop,
    Item::Record,
    Item::Snapshot,
    Item::Bookmark,
    Item::PictureInPicture,
    Item::Repeat,
    Item::Shuffle,
    Item::Audio,
    Item::Subtitles,
    Item::Settings,
    Item::Speed,
    Item::Back15,
    Item::Forward15,
    Item::Back10,
    Item::Forward10,
    Item::Chapters,
    Item::Bookmarks,
    Item::Replay,
    Item::Quit,
    Item::VlcSettings,
    Item::Properties,
    Item::Fullscreen,
    Item::GoHome,
    Item::Playlists,
];

fn button(cx: &mut VisualTestContext, item: Item) -> ButtonItem {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.button_item(item, cx)).expect("an icon button")
}

fn enabled(cx: &mut VisualTestContext, item: Item) -> bool {
    button(cx, item).enabled
}

#[gpui::test]
fn every_button_has_an_icon_and_a_label(cx: &mut TestAppContext) {
    let cx = open(cx);
    for item in BUTTONS {
        let button = button(cx, item);
        assert!(!button.icon.is_empty(), "{item:?}");
        assert!(!button.label.is_empty(), "{item:?}");
    }
    for item in [Item::Title, Item::SeekBar, Item::Volume, Item::PlayPause] {
        let main = main_view(cx);
        assert!(main.read_with(cx, |main, cx| main.button_item(item, cx)).is_none(), "{item:?}");
    }
}

#[gpui::test]
fn media_buttons_wait_for_media(cx: &mut TestAppContext) {
    let cx = open(cx);
    for item in [Item::Stop, Item::Audio, Item::Back15, Item::Replay, Item::Properties] {
        assert!(!enabled(cx, item), "{item:?} with nothing open");
    }
    assert!(enabled(cx, Item::Quit));
    assert!(enabled(cx, Item::Fullscreen));
    assert_eq!(button(cx, Item::GoHome).label, "Go Home");
    assert_eq!(button(cx, Item::Playlists).label, "Playlists");
}

#[gpui::test]
fn media_buttons_work_while_something_plays(cx: &mut TestAppContext) {
    let (cx, _playing) = open_playing(cx);
    for item in [Item::Stop, Item::Audio, Item::Forward10, Item::Chapters, Item::Replay] {
        assert!(enabled(cx, item), "{item:?} while playing");
    }
    assert!(matches!(button(cx, Item::Back15).press, Press::Skip(Way::Back, SKIP_15)));
    assert!(matches!(button(cx, Item::Subtitles).press, Press::Tracks(TrackKind::Subtitle)));
    assert!(!button(cx, Item::Record).selected, "not recording");
}

#[gpui::test]
fn shuffle_and_repeat_show_whether_they_are_on(cx: &mut TestAppContext) {
    let cx = open(cx);
    assert!(!button(cx, Item::Shuffle).selected);
    main_view(cx).update(cx, |main, _| main.queue.set_shuffle(true));
    assert!(button(cx, Item::Shuffle).selected);
    let before = button(cx, Item::Repeat);
    main_view(cx).update(cx, |main, _| main.queue.set_repeat(crate::queue::Repeat::All));
    let after = button(cx, Item::Repeat);
    assert!(!before.selected && after.selected);
}
