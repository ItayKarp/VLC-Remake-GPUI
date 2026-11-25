//! Test support for the main window's menus and panels: a window playing a clip on the mock
//! backend (never libVLC, never a sound), and reading and choosing menu entries as a person
//! does.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::engine::backend::BackendEvent;
use crate::engine::language::Language;
use crate::engine::media_info::Chapter;
use crate::engine::mock::{MockBackend, MockHandle, MockMedia};
use crate::engine::player::Player;
use crate::engine::tracks::{Track, TrackId, TrackKind, Tracks};
use crate::main_window::tests::{main_view, open};
use crate::opening::scratch::scratch_dir;
use crate::ui::menu::MenuItem;
use crate::video::Output;

/// The main window playing a clip on the mock backend, and the handle driving the mock.
pub(in crate::main_window) fn playing(
    cx: &mut TestAppContext,
    media: MockMedia,
) -> (&mut VisualTestContext, MockHandle) {
    let cx = open(cx);
    let mock = with_mock_player(cx, media);
    let clip = scratch_dir().join("Clip.mkv");
    std::fs::write(&clip, "not really a video").expect("the scratch folder is writable");
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.open(OpenRequest::replace(vec![Media::from(clip)]), window, cx);
    });
    cx.run_until_parked();
    (cx, mock)
}

/// Gives the window a player on the mock backend, as its first open would make one on libVLC.
pub(in crate::main_window) fn with_mock_player(
    cx: &mut VisualTestContext,
    media: MockMedia,
) -> MockHandle {
    let slot = std::rc::Rc::new(std::cell::RefCell::new(None));
    let out = slot.clone();
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        let backend = move |events| {
            let (mock, handle) = MockBackend::new(events, media);
            *out.borrow_mut() = Some(handle);
            Box::new(mock) as Box<dyn crate::engine::backend::Backend>
        };
        main.attach_player(Output::new(), backend, window, cx);
    });
    slot.borrow_mut().take().expect("the player made its backend")
}

/// The window's player.
pub(in crate::main_window) fn player(cx: &mut VisualTestContext) -> gpui::Entity<Player> {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.player.as_ref().map(|(player, _)| player.clone()))
        .expect("a player")
}

/// Sends `event` as the backend and lets the window take it in.
pub(in crate::main_window) fn emit(
    cx: &mut VisualTestContext,
    mock: &MockHandle,
    event: BackendEvent,
) {
    mock.emit(event);
    cx.run_until_parked();
}

/// Chapters named `titles`, a minute apart from the start.
pub(in crate::main_window) fn chapters(titles: &[&str]) -> BackendEvent {
    let minute = Duration::from_secs(60);
    let listed = titles
        .iter()
        .zip(0..)
        .map(|(title, index)| Chapter { title: Some((*title).to_owned()), start: minute * index });
    BackendEvent::Chapters(listed.collect())
}

/// One video track, English and Japanese audio with English playing, and English and French
/// subtitles, none shown.
pub(in crate::main_window) fn movie() -> Tracks {
    let track = |id, kind, language| Track {
        language: Language::parse(language),
        ..Track::new(TrackId(id), kind)
    };
    Tracks {
        all: vec![
            track(1, TrackKind::Video, ""),
            track(2, TrackKind::Audio, "eng"),
            track(3, TrackKind::Audio, "jpn"),
            track(4, TrackKind::Subtitle, "eng"),
            track(5, TrackKind::Subtitle, "fre"),
        ],
        video: Some(TrackId(1)),
        audio: Some(TrackId(2)),
        subtitle: None,
    }
}

/// The entry at `path` (labels from the top menu down through submenus).
pub(in crate::main_window) fn find<'a>(items: &'a [MenuItem], path: &[&str]) -> &'a MenuItem {
    let (first, rest) = path.split_first().expect("a path");
    let item = items
        .iter()
        .find(|item| label(item) == Some(*first))
        .unwrap_or_else(|| panic!("{first} missing from {:?}", labels(items)));
    match (rest.is_empty(), item) {
        (true, _) => item,
        (false, MenuItem::Submenu { items, .. }) => find(items, rest),
        (false, _) => panic!("{first} has no submenu"),
    }
}

/// The entries of the submenu `item`.
pub(in crate::main_window) fn submenu(item: &MenuItem) -> &[MenuItem] {
    match item {
        MenuItem::Submenu { items, .. } => items,
        _ => panic!("not a submenu"),
    }
}

/// An entry's label; separators have none.
pub(in crate::main_window) fn label(item: &MenuItem) -> Option<&str> {
    match item {
        MenuItem::Action { label, .. }
        | MenuItem::Check { label, .. }
        | MenuItem::Submenu { label, .. } => Some(label.as_ref()),
        MenuItem::Separator => None,
    }
}

/// The labels of `items`, separators left out.
pub(in crate::main_window) fn labels(items: &[MenuItem]) -> Vec<String> {
    items.iter().filter_map(label).map(str::to_owned).collect()
}

/// The labels of the checked entries among `items`.
pub(in crate::main_window) fn checked(items: &[MenuItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            MenuItem::Check { label, checked: true, .. } => Some(label.to_string()),
            _ => None,
        })
        .collect()
}

/// Whether `item` can be chosen: an enabled command, or a submenu with entries.
pub(in crate::main_window) fn enabled(item: &MenuItem) -> bool {
    match item {
        MenuItem::Action { disabled, .. } | MenuItem::Check { disabled, .. } => !disabled,
        MenuItem::Submenu { items, .. } => !items.is_empty(),
        MenuItem::Separator => false,
    }
}

/// Why `item` can't be chosen, as its hover says.
pub(in crate::main_window) fn reason(item: &MenuItem) -> Option<String> {
    match item {
        MenuItem::Action { reason, .. }
        | MenuItem::Check { reason, .. }
        | MenuItem::Submenu { reason, .. } => reason.as_ref().map(ToString::to_string),
        MenuItem::Separator => None,
    }
}

/// Chooses `item`, as a click on it does once the menu closes, and settles the app.
pub(in crate::main_window) fn choose(cx: &mut VisualTestContext, item: &MenuItem) {
    let on_select = match item {
        MenuItem::Action { on_select, .. } | MenuItem::Check { on_select, .. } => on_select.clone(),
        _ => panic!("{:?} can't be chosen", label(item)),
    };
    cx.update(|window, cx| on_select(window, cx));
    cx.run_until_parked();
}

/// The window's toasts, oldest first: each message and its buttons.
pub(in crate::main_window) fn toasts(cx: &mut VisualTestContext) -> Vec<(String, Vec<String>)> {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| main.toasts.read(cx).shown())
}

/// The messages of the window's toasts, oldest first.
pub(in crate::main_window) fn toast_messages(cx: &mut VisualTestContext) -> Vec<String> {
    toasts(cx).into_iter().map(|(message, _)| message).collect()
}

/// Presses the newest toast's button labelled `label`, and settles the app.
pub(in crate::main_window) fn press_toast(cx: &mut VisualTestContext, label: &str) {
    let main = main_view(cx);
    let toasts = main.read_with(cx, |main, _| main.toasts.clone());
    cx.update(|window, cx| toasts.update(cx, |toasts, cx| toasts.press_button(label, window, cx)));
    cx.run_until_parked();
}
