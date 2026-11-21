//! The `…` main menu (S0.5.4, S2.9.1): the designed list from [`MAIN_MENU`], with each
//! command's name and shortcut from the keymap. Commands whose stories haven't shipped are left
//! out (S2.9.4); one nothing can run right now is greyed out and says why on hover.

use gpui::{Action, App, Context, Div, WeakEntity, Window, prelude::*};

use super::MainWindow;
use crate::app::request::OpenRequest;
use crate::keymap;
use crate::keymap::actions::{About, FromMenuBar};
use crate::keymap::commands;
use crate::keymap::menus::{self, MAIN_MENU, MainMenuEntry};
use crate::opening::recent;
use crate::ui::menu::MenuItem;

impl MainWindow {
    /// Adds the handlers for Keyboard Shortcuts, About VLC and the macOS menu bar's player
    /// commands.
    pub(super) fn menu_actions(root: Div, cx: &mut Context<Self>) -> Div {
        Self::shortcuts_dialog_action(root, cx)
            .on_action(cx.listener(|this, _: &About, window, cx| this.show_about(window, cx)))
            .on_action(cx.listener(Self::run_from_menu_bar))
    }

    /// A player command chosen from the macOS menu bar (S2.9.5): runs it as the video menu does.
    fn run_from_menu_bar(
        &mut self,
        chosen: &FromMenuBar,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(command) = commands::by_label(chosen.command) {
            self.perform(command.action, window, cx);
        }
    }
}

/// The main menu's items as they stand now, for the main window `main`.
pub(super) fn items(main: &WeakEntity<MainWindow>, window: &Window, cx: &mut App) -> Vec<MenuItem> {
    MAIN_MENU.iter().filter_map(|entry| item(entry, main, window, cx)).collect()
}

fn item(
    entry: &MainMenuEntry,
    main: &WeakEntity<MainWindow>,
    window: &Window,
    cx: &mut App,
) -> Option<MenuItem> {
    match entry {
        MainMenuEntry::Command { action, .. } if menus::is_pending(*action) => None,
        MainMenuEntry::Command { action, icon } => Some(command(*action, icon, window, cx)),
        MainMenuEntry::Recent { icon } => Some(recent_menu(main, cx).icon(icon)),
    }
}

/// The app command `action`, run by dispatching it as its shortcut would.
fn command(
    action: &'static dyn Action,
    icon: &'static str,
    window: &Window,
    cx: &mut App,
) -> MenuItem {
    let available = window.is_action_available(action, cx) || cx.is_action_available(action);
    dispatching(action, menus::label(action), cx)
        .icon(icon)
        .disabled(!available)
        .why_disabled("Not available here")
}

/// An item that dispatches `action` (an app command) under `label`, with its shortcut from the
/// keymap.
pub(super) fn dispatching(action: &'static dyn Action, label: &'static str, cx: &App) -> MenuItem {
    MenuItem::action(label, move |window, cx| {
        window.dispatch_action(action.boxed_clone(), cx);
    })
    .maybe_shortcut(keymap::shortcut(action, cx))
}

/// Open Recent: the recent media, newest first, then Clear Recent. Empty (so disabled, saying
/// why) when there is nothing recent or history is off.
pub(super) fn recent_menu(main: &WeakEntity<MainWindow>, cx: &App) -> MenuItem {
    MenuItem::submenu("Open Recent", recent_items(main, cx)).why_disabled("Nothing opened recently")
}

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
