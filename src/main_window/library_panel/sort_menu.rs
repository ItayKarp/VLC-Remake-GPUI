//! The header's Sort button and menu (S4.7.1, S4.7.2; `sorting_options`): the sorts the current
//! place offers, the current one checked. An orange dot on the button says the place isn't in
//! its default order. Each place remembers its own sort across launches.
//!
//! Routes that list no cards (the media folders, Music's categories, playlists) don't sort, and
//! the button shows disabled there.

use gpui::{AnyElement, App, ClickEvent, Context, Div, Window, div, prelude::*, px};

use super::header::{BUTTON, ICON};
use super::{MainWindow, OpenMenu};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::navigation::sort::{self, Sort, Sortable};
use crate::settings;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};

/// How far below its button the menu opens.
const MENU_GAP: f32 = 4.;
/// The not-default dot: its size, and how far in from the button's top-right corner it sits.
const DOT: f32 = 6.;
const DOT_INSET: f32 = 3.;

impl MainWindow {
    /// How the current route sorts; `None` when it doesn't.
    pub(super) fn library_sort(&self, cx: &App) -> Option<Sort> {
        sort::of(self.library_panel.history.current(), &settings::get(cx).local.library_sorts)
    }

    /// Sort, with the dot while the place isn't in its default order.
    pub(super) fn library_sort_button(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let sortable = Sortable::of(self.library_panel.history.current());
        let sort = self.library_sort(cx);
        let changed = sortable.zip(sort).is_some_and(|(place, sort)| sort != place.default_sort());
        let label = sort.map_or_else(
            || tr!("Sort").to_owned(),
            |sort| tr!("Sort: {sort}", sort = sort.label()),
        );
        let button = IconButton::new("library-sort", icons::SORT, label)
            .size(BUTTON, ICON)
            .disabled(sortable.is_none())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_library_sort_menu(window, cx);
            }));
        let dot = div()
            .absolute()
            .top(px(DOT_INSET))
            .inset_end(px(DOT_INSET))
            .size(px(DOT))
            .rounded_full()
            .bg(p.accent);
        div()
            .relative()
            .child(self.library_panel.sort_button.element())
            .child(button)
            .when(changed, |this| this.child(dot))
    }

    /// Opens or closes the Sort menu under its button.
    fn toggle_library_sort_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library_panel.sort_menu.take().is_some() {
            cx.notify();
            return;
        }
        let sortable = Sortable::of(self.library_panel.history.current());
        let (Some(sortable), Some(current)) = (sortable, self.library_sort(cx)) else { return };
        let main = cx.entity().downgrade();
        let item = |sort: Sort| {
            let main = main.clone();
            let item = MenuItem::radio(sort.label(), sort == current, move |_, cx| {
                main.update(cx, |main, cx| main.set_library_sort(sort, cx)).ok();
            });
            match sort.hint() {
                Some(hint) => item.hint(hint),
                None => item,
            }
        };
        let items = sortable.options().into_iter().map(item).collect();
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &gpui::DismissEvent, cx| {
            this.library_panel.sort_menu = None;
            cx.notify();
        });
        self.library_panel.sort_menu = Some((menu, subscription));
        cx.notify();
    }

    /// Sorts the current place by `sort` from now on, keeping the keyboard's item in sight.
    pub(super) fn set_library_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        let route = self.library_panel.history.current().clone();
        settings::update(cx, |s| sort::remember(&route, sort, &mut s.local.library_sorts));
        self.sort_listing(sort);
        let shape = self.library_shape(cx);
        let line = self.cursor_index(cx).map_or(0, |index| shape.line_of(index));
        self.library_panel.scroll.scroll_to_item(line, gpui::ScrollStrategy::Center);
        cx.notify();
    }
}

/// The Sort menu under its button, while open.
pub(super) fn render(menu: &OpenMenu, button: &BoundsTracker) -> Option<AnyElement> {
    let (menu, _) = menu.as_ref()?;
    Some(popover::attached(button.bounds()?, Side::Below, MENU_GAP, 1, menu.clone()))
}
