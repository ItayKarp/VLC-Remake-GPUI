//! The `…` main menu (S0.5.4): the designed list from [`MAIN_MENU`], with each
//! command's name and shortcut from the keymap. Commands nothing handles yet are
//! greyed out rather than hidden, so the menu keeps its designed shape.

use gpui::{Action, App, Window};

use crate::keymap;
use crate::keymap::menus::{self, MAIN_MENU, MainMenuEntry};
use crate::ui::menu::MenuItem;

/// The main menu's items as they stand now.
pub(super) fn items(window: &Window, cx: &mut App) -> Vec<MenuItem> {
    MAIN_MENU.iter().map(|entry| item(entry, window, cx)).collect()
}

fn item(entry: &MainMenuEntry, window: &Window, cx: &mut App) -> MenuItem {
    match entry {
        MainMenuEntry::Command { action, icon } => {
            let available = is_available(*action, window, cx);
            let action = action.boxed_clone();
            let shortcut = keymap::shortcut(&*action, cx);
            let label = menus::label(&*action);
            let item = MenuItem::action(label, move |window, cx| {
                window.dispatch_action(action.boxed_clone(), cx);
            })
            .icon(icon)
            .disabled(!available);
            match shortcut {
                Some(shortcut) => item.shortcut(shortcut),
                None => item,
            }
        }
        // Their contents aren't designed yet: shown, but empty (and so disabled).
        MainMenuEntry::Submenu { label, icon } => MenuItem::submenu(*label, Vec::new()).icon(icon),
    }
}

/// Whether the main window or the app handles `action`.
fn is_available(action: &dyn Action, window: &Window, cx: &mut App) -> bool {
    window.is_action_available(action, cx) || cx.is_action_available(action)
}
