//! The menus (S0.5.4): the in-window main menu from the design
//! (`docs/ui/player/menus/main_menu/menu.png`) on Windows and Linux, and the
//! macOS menu bar. Both list commands from [`super::commands`], so their names and
//! shortcuts come from the command table and the keymap.

use gpui::{Action, Menu, MenuItem, SharedString};

use super::actions as act;
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
    /// A submenu whose contents aren't designed yet (Profile).
    Submenu {
        /// The text shown.
        label: &'static str,
        /// The icon path.
        icon: &'static str,
    },
}

const fn command(action: &'static dyn Action, icon: &'static str) -> MainMenuEntry {
    MainMenuEntry::Command { action, icon }
}

/// The main menu, top to bottom as designed, plus "Run Setup Again" (S0.2.5) and
/// "Copy Debug Info" (S0.7.4), which the Help menu holds on macOS.
pub const MAIN_MENU: &[MainMenuEntry] = &[
    command(&act::OpenFile, icons::FILE),
    command(&act::OpenFolder, icons::FOLDER),
    command(&act::OpenNetworkStream, icons::STREAMING),
    command(&act::OpenDisc, icons::DISC),
    command(&act::OpenCaptureDevice, icons::CAPTURE_DEVICE),
    command(&act::Convert, icons::CONVERT),
    MainMenuEntry::Recent { icon: icons::RECENT },
    MainMenuEntry::Submenu { label: "Profile", icon: icons::PROFILE },
    command(&act::ShowKeyboardShortcuts, icons::HOTKEYS),
    command(&act::CustomizePlayer, icons::PAINT),
    command(&act::OpenSettings, icons::SETTINGS),
    command(&act::RunSetupAgain, icons::THEME_SYSTEM),
    command(&act::About, icons::INFO),
    command(&act::CopyDebugInfo, icons::LIST),
];

/// The command table's label for `action`.
pub fn label(action: &dyn Action) -> &'static str {
    commands::find(action).map_or("", |command| command.label)
}

/// The macOS menu bar. Only commands with modifier shortcuts appear in it: macOS
/// would fire a plain-letter key equivalent even while a text field has focus.
/// GPUI shows each item's shortcut from the keymap and disables items no
/// window handles yet.
pub fn mac_menu_bar() -> Vec<Menu> {
    vec![
        Menu::new("VLC").items([
            item(&act::About, "About VLC"),
            MenuItem::separator(),
            item(&act::OpenSettings, "Settings…"),
            item(&act::ShowKeyboardShortcuts, "Keyboard Shortcuts"),
            item(&act::CustomizePlayer, "Customize Player…"),
            MenuItem::separator(),
            item(&act::Quit, "Quit VLC"),
        ]),
        Menu::new("File").items([
            item(&act::OpenFile, "Open File(s)…"),
            item(&act::OpenFolder, "Open Folder…"),
            item(&act::OpenNetworkStream, "Open Network Stream…"),
            item(&act::OpenDisc, "Open Disc…"),
            item(&act::OpenCaptureDevice, "Open Capture Device…"),
            MenuItem::separator(),
            item(&act::Convert, "Convert…"),
        ]),
        Menu::new("Window").items([item(&act::CloseWindow, "Close Window")]),
        Menu::new("Help").items([
            item(&act::ShowKeyboardShortcuts, "Keyboard Shortcuts"),
            item(&act::RunSetupAgain, "Run Setup Again…"),
            MenuItem::separator(),
            item(&act::CopyDebugInfo, "Copy Debug Info"),
        ]),
    ]
}

fn item(action: &dyn Action, name: &'static str) -> MenuItem {
    MenuItem::Action {
        name: SharedString::from(name),
        action: action.boxed_clone(),
        os_action: None,
        checked: false,
        disabled: false,
    }
}
