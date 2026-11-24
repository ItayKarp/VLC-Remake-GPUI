//! The menus (S0.5.4, S2.9): the in-window main menu from the design
//! (`docs/ui/player/menus/main_menu/menu.png`) and the macOS menu bar. Both list commands from
//! [`super::commands`], so their names and shortcuts come from the command table and the keymap.
//!
//! Commands whose stories haven't shipped ([`PENDING`]) are left out of every menu (S2.9.4)
//! rather than shown disabled; a disabled item means "not now", with a reason on hover.

use gpui::{Action, Menu, MenuItem, SharedString};

use super::actions::{self as act, FromMenuBar};
use super::commands;
use crate::assets::icons;

/// One row of the in-window main menu.
pub enum MainMenuEntry {
    /// A command, with its icon.
    Command {
        /// The command's action; its label comes from the command table.
        action: &'static dyn Action,
        /// The icon path.
        icon: &'static str,
    },
    /// Open Recent (S1.3.5): the recent media list and Clear Recent.
    Recent {
        /// The icon path.
        icon: &'static str,
    },
    /// Profile (S8.8): the profiles, the one in use checked, and + New.
    Profile {
        /// The icon path.
        icon: &'static str,
    },
}

const fn command(action: &'static dyn Action, icon: &'static str) -> MainMenuEntry {
    MainMenuEntry::Command { action, icon }
}

/// The main menu, top to bottom as designed, plus "Run Setup Again" (S0.2.5) and
/// "Copy Debug Info" (S0.7.4), which the Help menu holds on macOS. The pending commands are
/// filtered out when the menu opens.
pub const MAIN_MENU: &[MainMenuEntry] = &[
    command(&act::OpenFile, icons::FILE),
    command(&act::OpenFolder, icons::FOLDER),
    command(&act::OpenNetworkStream, icons::STREAMING),
    command(&act::OpenDisc, icons::DISC),
    command(&act::OpenCaptureDevice, icons::CAPTURE_DEVICE),
    command(&act::Convert, icons::CONVERT),
    command(&act::Stream, icons::STREAMING),
    MainMenuEntry::Recent { icon: icons::RECENT },
    MainMenuEntry::Profile { icon: icons::PROFILE },
    command(&act::ShowKeyboardShortcuts, icons::HOTKEYS),
    command(&act::CustomizePlayer, icons::PAINT),
    command(&act::OpenSettings, icons::SETTINGS),
    command(&act::RunSetupAgain, icons::THEME_SYSTEM),
    command(&act::About, icons::INFO),
    command(&act::CopyDebugInfo, icons::LIST),
];

/// Commands that menus and the Keyboard Shortcuts dialog leave out until their stories ship
/// (S2.9.4); their keys say "Coming soon" in debug builds. Remove a command from here when it
/// gets its effect.
pub const PENDING: &[&dyn Action] = &[];

/// Whether `action` is one of the [`PENDING`] commands.
pub fn is_pending(action: &dyn Action) -> bool {
    PENDING.iter().any(|pending| pending.partial_eq(action))
}

/// The command table's label for `action`, in the interface language.
pub fn label(action: &dyn Action) -> &'static str {
    commands::find(action).map_or("", commands::Command::title)
}

/// The macOS menu bar (S2.9.5): the main menu's commands and the video menu's player commands.
/// GPUI shows each item's shortcut from the keymap and disables items no window handles. Player
/// commands with plain-key shortcuts go through [`FromMenuBar`] and show no shortcut: macOS
/// would fire a plain-letter key equivalent even while a text field has focus.
pub fn mac_menu_bar() -> Vec<Menu> {
    vec![
        app_menu(),
        file_menu(),
        playback_menu(),
        audio_menu(),
        subtitle_menu(),
        menu(
            tr!("View"),
            [
                plain(&act::ToggleFullscreen, tr!("Toggle Fullscreen")),
                item(&act::HideInterface, tr!("Hide Interface")),
                item(&act::PictureInPicture, tr!("Picture in Picture")),
                plain(&act::Snapshot, tr!("Take Snapshot")),
            ],
        ),
        menu(tr!("Window"), [item(&act::CloseWindow, tr!("Close Window"))]),
        menu(
            tr!("Help"),
            [
                item(&act::ShowKeyboardShortcuts, tr!("Keyboard Shortcuts")),
                item(&act::RunSetupAgain, tr!("Run Setup Again…")),
                SEPARATOR,
                item(&act::CopyDebugInfo, tr!("Copy Debug Info")),
            ],
        ),
    ]
}

/// Audio: the audio track and the volume.
fn audio_menu() -> Menu {
    menu(
        tr!("Audio"),
        [
            plain(&act::CycleAudioTrack, tr!("Next Audio Track")),
            SEPARATOR,
            plain(&act::VolumeUp, tr!("Increase Volume")),
            plain(&act::VolumeDown, tr!("Decrease Volume")),
        ],
    )
}

/// Subtitle: the subtitle track, adding a file, and the delay (S6.3.2).
fn subtitle_menu() -> Menu {
    menu(
        tr!("Subtitle"),
        [
            plain(&act::CycleSubtitleTrack, tr!("Next Subtitle Track")),
            item(&act::AddSubtitleFile, tr!("Add Subtitle File…")),
            SEPARATOR,
            plain(&act::SubtitleDelayLater, tr!("Increase Delay")),
            plain(&act::SubtitleDelayEarlier, tr!("Decrease Delay")),
        ],
    )
}

fn app_menu() -> Menu {
    menu(
        "VLC",
        [
            item(&act::About, tr!("About VLC")),
            SEPARATOR,
            item(&act::OpenSettings, tr!("Settings…")),
            item(&act::ShowKeyboardShortcuts, tr!("Keyboard Shortcuts")),
            item(&act::CustomizePlayer, tr!("Customize Player…")),
            SEPARATOR,
            item(&act::Quit, tr!("Quit VLC")),
        ],
    )
}

fn file_menu() -> Menu {
    menu(
        tr!("File"),
        [
            item(&act::OpenFile, tr!("Open File(s)…")),
            item(&act::OpenFolder, tr!("Open Folder…")),
            item(&act::OpenFromClipboard, tr!("Open from Clipboard")),
            item(&act::OpenNetworkStream, tr!("Open Network Stream…")),
            item(&act::OpenDisc, tr!("Open Disc…")),
            item(&act::OpenCaptureDevice, tr!("Open Capture Device…")),
            SEPARATOR,
            item(&act::NewPlaylist, tr!("New Playlist…")),
            item(&act::SaveQueueAsPlaylist, tr!("Save Queue as Playlist…")),
            item(&act::ImportPlaylist, tr!("Import Playlist…")),
            SEPARATOR,
            item(&act::Convert, tr!("Convert…")),
            item(&act::Stream, tr!("Stream…")),
        ],
    )
}

fn playback_menu() -> Menu {
    menu(
        tr!("Playback"),
        [
            plain(&act::PlayPause, tr!("Play / Pause")),
            plain(&act::Stop, tr!("Stop")),
            plain(&act::PreviousItem, tr!("Previous Item")),
            plain(&act::NextItem, tr!("Next Item")),
            plain(&act::PreviousChapter, tr!("Previous Chapter")),
            plain(&act::NextChapter, tr!("Next Chapter")),
            plain(&act::NextProgram, tr!("Next Program")),
            plain(&act::DiscMenu, tr!("Disc Menu")),
            plain(&act::LoopAToB, tr!("A-B Loop")),
            item(&act::AddBookmark, tr!("Add Bookmark")),
            plain(&act::Record, tr!("Record")),
            SEPARATOR,
            plain(&act::Faster, tr!("Faster")),
            plain(&act::Slower, tr!("Slower")),
            plain(&act::NormalSpeed, tr!("Normal Speed")),
            SEPARATOR,
            plain(&act::SeekForward, tr!("Jump Forward")),
            plain(&act::SeekBackward, tr!("Jump Backward")),
        ],
    )
}

/// A menu of the `items` that aren't pending, without separators left at an end or doubled.
fn menu<const N: usize>(name: &'static str, items: [Option<MenuItem>; N]) -> Menu {
    let mut kept: Vec<MenuItem> = Vec::with_capacity(N);
    for item in items.into_iter().flatten() {
        let is_separator = matches!(item, MenuItem::Separator);
        let follows_separator = kept.last().is_none_or(|last| matches!(last, MenuItem::Separator));
        if !(is_separator && follows_separator) {
            kept.push(item);
        }
    }
    if kept.last().is_some_and(|last| matches!(last, MenuItem::Separator)) {
        kept.pop();
    }
    Menu::new(name).items(kept)
}

/// A line between groups of a bar menu.
const SEPARATOR: Option<MenuItem> = Some(MenuItem::Separator);

/// `action` under `name`, unless it is pending.
fn item(action: &dyn Action, name: &'static str) -> Option<MenuItem> {
    (!is_pending(action)).then(|| menu_item(action.boxed_clone(), name))
}

/// The player command `action` through [`FromMenuBar`], so it gets no key equivalent.
fn plain(action: &dyn Action, name: &'static str) -> Option<MenuItem> {
    let command = commands::find(action)?;
    item(action, name).map(|_| menu_item(Box::new(FromMenuBar { command: command.label }), name))
}

fn menu_item(action: Box<dyn Action>, name: &'static str) -> MenuItem {
    MenuItem::Action {
        name: SharedString::from(name),
        action,
        os_action: None,
        checked: false,
        disabled: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_commands_stay_out_of_the_bar_and_separators_stay_tidy() {
        let bar = mac_menu_bar();
        let names: Vec<_> = bar.iter().map(|menu| menu.name.to_string()).collect();
        assert!(names.contains(&"Playback".to_owned()), "{names:?}");
        for menu in &bar {
            let items = &menu.items;
            assert!(!items.is_empty(), "{} is empty", menu.name);
            assert!(!matches!(items.first(), Some(MenuItem::Separator)), "{}", menu.name);
            assert!(!matches!(items.last(), Some(MenuItem::Separator)), "{}", menu.name);
            let actions = items.iter().filter_map(|item| match item {
                MenuItem::Action { action, .. } => Some(action),
                _ => None,
            });
            for action in actions {
                assert!(!is_pending(action.as_ref()), "{} lists a pending command", menu.name);
            }
        }
    }

    #[test]
    fn plain_key_commands_reach_the_bar_without_their_keys() {
        let Some(MenuItem::Action { action, .. }) = plain(&act::PlayPause, "Play / Pause") else {
            panic!("Play / Pause is in the bar");
        };
        assert!(action.partial_eq(&FromMenuBar { command: label(&act::PlayPause) }));
        assert!(commands::by_label(label(&act::PlayPause)).is_some());
        assert!(plain(&act::NextProgram, "Next Program").is_some(), "Next Program shipped (S8.9)");
        assert!(item(&act::Convert, "Convert…").is_some(), "Convert shipped (S8.10)");
    }
}
