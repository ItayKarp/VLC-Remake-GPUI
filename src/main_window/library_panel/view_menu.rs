//! The header's View button and menu (S4.6.4, S4.7.4; `view_options`): Grid View or List View,
//! the current one checked. The button shows the current view's icon. Each section remembers
//! its own view across launches.

use gpui::{AnyElement, App, Context, Window};

use super::grid::COLUMNS;
use super::{MainWindow, OpenMenu};
use crate::assets::icons;
use crate::navigation::view::View;
use crate::settings;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};

/// How far below its button the menu opens.
const MENU_GAP: f32 = 4.;

impl MainWindow {
    /// How the current route's section shows its items.
    pub(super) fn library_view(&self, cx: &App) -> View {
        let section = self.library_panel.history.current().section();
        View::of(section, &settings::get(cx).local.library_grid)
    }

    /// Opens or closes the View menu under its button.
    pub(super) fn toggle_view_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library_panel.view_menu.take().is_some() {
            cx.notify();
            return;
        }
        let current = self.library_view(cx);
        let main = cx.entity().downgrade();
        let item = |view: View, label: &'static str, icon: &'static str| {
            let main = main.clone();
            MenuItem::radio(label, view == current, move |_, cx| {
                main.update(cx, |main, cx| main.set_library_view(view, cx)).ok();
            })
            .icon(icon)
        };
        let items = vec![
            item(View::Grid, "Grid View", icons::GRID_VIEW),
            item(View::List, "List View", icons::LIST_VIEW),
        ];
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            this.library_panel.view_menu = None;
            cx.notify();
        });
        self.library_panel.view_menu = Some((menu, subscription));
        cx.notify();
    }

    /// Shows the current section as `view` from now on, keeping the keyboard's item in sight.
    pub(super) fn set_library_view(&mut self, view: View, cx: &mut Context<Self>) {
        let section = self.library_panel.history.current().section();
        settings::update(cx, |s| view.remember(section, &mut s.local.library_grid));
        let line = self.cursor_index(cx).map_or(0, |index| line_of(index, view));
        self.library_panel.scroll.scroll_to_item(line, gpui::ScrollStrategy::Center);
        cx.notify();
    }
}

/// The line of the listing item `index` is on when shown as `view`.
pub(super) fn line_of(index: usize, view: View) -> usize {
    match view {
        View::List => index,
        View::Grid => index / COLUMNS,
    }
}

/// The View menu under its button, while open.
pub(super) fn render(menu: &OpenMenu, button: &BoundsTracker) -> Option<AnyElement> {
    let (menu, _) = menu.as_ref()?;
    Some(popover::attached(button.bounds()?, Side::Below, MENU_GAP, 1, menu.clone()))
}

/// The View button's icon: the current view's.
pub(super) fn icon(view: View) -> &'static str {
    match view {
        View::List => icons::LIST_VIEW,
        View::Grid => icons::GRID_VIEW,
    }
}
