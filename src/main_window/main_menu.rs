//! The `…` main menu (S0.5.4): the designed list from [`MAIN_MENU`], with each
//! command's name and shortcut from the keymap. Commands nothing handles yet are
//! greyed out rather than hidden, so the menu keeps its designed shape.

use gpui::{Action, App, WeakEntity, Window};

use super::MainWindow;
use crate::app::request::OpenRequest;
use crate::keymap;
use crate::keymap::menus::{self, MAIN_MENU, MainMenuEntry};
use crate::opening::recent;
use crate::ui::menu::MenuItem;

/// The main menu's items as they stand now, for the main window `main`.
pub(super) fn items(main: &WeakEntity<MainWindow>, window: &Window, cx: &mut App) -> Vec<MenuItem> {
    MAIN_MENU.iter().map(|entry| item(entry, main, window, cx)).collect()
}

fn item(
    entry: &MainMenuEntry,
    main: &WeakEntity<MainWindow>,
    window: &Window,
    cx: &mut App,
) -> MenuItem {
    match entry {
        MainMenuEntry::Command { action, icon } => command(*action, icon, window, cx),
        MainMenuEntry::Recent { icon } => {
            MenuItem::submenu("Open Recent", recent_items(main, cx)).icon(icon)
        }
        // Their contents aren't designed yet: shown, but empty (and so disabled).
        MainMenuEntry::Submenu { label, icon } => MenuItem::submenu(*label, Vec::new()).icon(icon),
    }
}

fn command(
    action: &'static dyn Action,
    icon: &'static str,
    window: &Window,
    cx: &mut App,
) -> MenuItem {
    let available = is_available(action, window, cx);
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

/// The recent media, newest first, then Clear Recent. Empty (so the submenu is disabled) when
/// there is nothing recent or history is off.
fn recent_items(main: &WeakEntity<MainWindow>, cx: &App) -> Vec<MenuItem> {
    let recent = recent::items(cx);
    if recent.is_empty() {
        return Vec::new();
    }
    let mut items: Vec<MenuItem> = recent
        .into_iter()
        .map(|media| {
            let main = main.clone();
            MenuItem::action(media.display_name(), move |window, cx| {
                let request = OpenRequest::replace(vec![media.clone()]);
                main.update(cx, |main, cx| main.open(request, window, cx)).ok();
            })
        })
        .collect();
    items.push(MenuItem::Separator);
    items.push(MenuItem::action("Clear Recent", |_, cx| recent::clear(cx)));
    items
}

/// Whether the main window or the app handles `action`.
fn is_available(action: &dyn Action, window: &Window, cx: &mut App) -> bool {
    window.is_action_available(action, cx) || cx.is_action_available(action)
}
