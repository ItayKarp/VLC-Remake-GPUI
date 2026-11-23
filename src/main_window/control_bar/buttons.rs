//! What each icon button a bar can show looks like and does: the designed bar's buttons and the
//! ones Customize Player adds (S8.7.2).

use std::time::Duration;

use gpui::{Action, App};

use crate::assets::icons;
use crate::engine::tracks::TrackKind;
use crate::keymap::actions as act;
use crate::keymap::menus;
use crate::library;
use crate::main_window::MainWindow;
use crate::main_window::play_order::repeat_button_look;
use crate::main_window::queue_panel::ListTab;
use crate::main_window::seeking::Way;
use crate::player_layout::Item;

/// What pressing an icon button does.
#[derive(Clone, Copy)]
pub(super) enum Press {
    /// Runs a player command.
    Run(&'static dyn Action),
    /// Dispatches an app command (Quit, Settings, Properties), as its menu item does.
    Dispatch(&'static dyn Action),
    /// Opens (or closes) the track popover's tab of this kind.
    Tracks(TrackKind),
    /// Opens (or closes) the settings panel.
    Settings,
    /// Jumps this far this way.
    Skip(Way, Duration),
    /// Opens (or closes) the queue panel on this tab.
    Tab(ListTab),
    /// Opens the library at this place.
    Library(LibraryPlace),
    /// Plays the media again from its start.
    Replay,
}

/// A library place a button opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LibraryPlace {
    /// Home (S4.9).
    Home,
    /// The saved playlists (S3.5).
    Playlists,
}

/// An icon button's look and what it does.
#[derive(Clone, Copy)]
pub(super) struct ButtonItem {
    /// Its icon.
    pub(super) icon: &'static str,
    /// Its tooltip, and its name in the overflow menu.
    pub(super) label: &'static str,
    /// What pressing it does.
    pub(super) press: Press,
    /// Whether it does anything now.
    pub(super) enabled: bool,
    /// It shows as on (Repeat, Shuffle and Record while they are).
    pub(super) selected: bool,
}

/// How far the 10 s buttons jump, as their icons say.
const SKIP_10: Duration = Duration::from_secs(10);
/// How far the 15 s buttons jump, as their icons say (S8.1.2).
const SKIP_15: Duration = Duration::from_secs(15);

impl MainWindow {
    /// The icon button `item` is, if it is one.
    pub(super) fn button_item(&self, item: Item, cx: &App) -> Option<ButtonItem> {
        if let Some(button) = self.media_button(item, cx) {
            return Some(button);
        }
        let (icon, action, enabled): (_, &'static dyn Action, _) = match item {
            Item::Previous => (icons::PREVIOUS, &act::PreviousItem, self.can_go_back(cx)),
            Item::Next => (icons::NEXT, &act::NextItem, self.can_go_next()),
            Item::List => (icons::LIST, &act::ToggleQueue, true),
            Item::Stop => (icons::STOP, &act::Stop, self.has_active_media(cx)),
            Item::Record => (icons::RECORD, &act::Record, self.has_active_media(cx)),
            Item::Snapshot => (icons::SNAPSHOT, &act::Snapshot, self.has_active_media(cx)),
            Item::Bookmark => (icons::BOOKMARK, &act::AddBookmark, self.has_active_media(cx)),
            Item::PictureInPicture => (icons::PIP, &act::PictureInPicture, true),
            Item::Repeat => return Some(self.repeat_button()),
            Item::Shuffle => (icons::SHUFFLE, &act::ToggleShuffle, true),
            _ => return self.app_button(item, cx),
        };
        let enabled = enabled && Self::handles(action);
        let selected = match item {
            Item::Shuffle => self.queue.is_shuffled(),
            Item::Record => self.is_recording(cx),
            _ => false,
        };
        let label = menus::label(action);
        Some(ButtonItem { icon, label, press: Press::Run(action), enabled, selected })
    }

    /// The buttons that act on what plays, enabled while something does.
    fn media_button(&self, item: Item, cx: &App) -> Option<ButtonItem> {
        let (icon, label, press) = match item {
            Item::Audio => (icons::AUDIO, "Audio", Press::Tracks(TrackKind::Audio)),
            Item::Subtitles => (icons::SUBTITLES, "Subtitles", Press::Tracks(TrackKind::Subtitle)),
            Item::Settings => (icons::SETTINGS, "Playback settings", Press::Settings),
            Item::Speed => (icons::SPEED, "Playback speed", Press::Settings),
            Item::Back15 => (icons::BACK_15, "Back 15 s", Press::Skip(Way::Back, SKIP_15)),
            Item::Forward15 => {
                (icons::FORWARD_15, "Forward 15 s", Press::Skip(Way::Forward, SKIP_15))
            }
            Item::Back10 => (icons::BACK_10, "Back 10 s", Press::Skip(Way::Back, SKIP_10)),
            Item::Forward10 => {
                (icons::FORWARD_10, "Forward 10 s", Press::Skip(Way::Forward, SKIP_10))
            }
            Item::Chapters => (icons::CHAPTERS, "Chapters", Press::Tab(ListTab::Chapters)),
            Item::Bookmarks => (icons::BOOKMARKS, "Bookmarks", Press::Tab(ListTab::Bookmarks)),
            Item::Replay => (icons::REPLAY, "Replay", Press::Replay),
            _ => return None,
        };
        let enabled = match item {
            Item::Replay => self.loaded_player(cx).is_some(),
            _ => self.has_active_media(cx),
        };
        Some(ButtonItem { icon, label, press, enabled, selected: false })
    }

    /// The buttons that open other parts of the app, or quit it.
    fn app_button(&self, item: Item, cx: &App) -> Option<ButtonItem> {
        let (icon, press, enabled) = match item {
            Item::Quit => (icons::QUIT, Press::Dispatch(&act::Quit), true),
            Item::VlcSettings => (icons::SETTINGS, Press::Dispatch(&act::OpenSettings), true),
            Item::Properties => {
                (icons::INFO, Press::Dispatch(&act::ShowProperties), self.has_active_media(cx))
            }
            Item::GoHome => (icons::HOME, Press::Library(LibraryPlace::Home), true),
            Item::Playlists => (icons::PLAYLIST, Press::Library(LibraryPlace::Playlists), true),
            _ => return None,
        };
        let label = match press {
            Press::Dispatch(action) => menus::label(action),
            Press::Library(LibraryPlace::Home) => "Go Home",
            _ => "Playlists",
        };
        let enabled = enabled && (!matches!(press, Press::Library(_)) || library::is_enabled(cx));
        Some(ButtonItem { icon, label, press, enabled, selected: false })
    }

    fn repeat_button(&self) -> ButtonItem {
        let (icon, selected, label) = repeat_button_look(self.queue.repeat());
        ButtonItem { icon, label, press: Press::Run(&act::CycleRepeat), enabled: true, selected }
    }
}
