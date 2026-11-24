//! The panel's header (`library/sidebar/*`): Back and Forward with their real enabled states
//! (S4.5.2), the breadcrumb, `•••` and pop-out; under them the item count (S4.5.5), with "Not in
//! library" for a folder outside every media folder, and Sort, View and Search; then a divider.
//!
//! Sort and View open their menus ([`sort_menu`](super::sort_menu),
//! [`view_menu`](super::view_menu)), and Search a field under the count row
//! ([`search_field`](super::search_field)). Pop-out switches to full library mode (S4.10.2),
//! whose header is one row of the same parts ([`full`](super::full)). `•••` (its menu isn't
//! designed) shows disabled until its story.

use gpui::{Action, App, ClickEvent, Context, Div, SharedString, div, prelude::*, px};

use super::INSET;
use super::MainWindow;
use super::view_menu;
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::keymap;
use crate::keymap::actions::{GoBack, GoForward};
use crate::navigation::breadcrumb::Trail;
use crate::navigation::history::Step;
use crate::navigation::item_count;
use crate::navigation::route::Route;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;

/// The header's buttons: 28 px, 8 px apart, 18 px in from the panel's sides (`player_02`).
pub(super) const BUTTON: f32 = 28.;
pub(super) const ICON: f32 = 20.;
pub(super) const GAP: f32 = 8.;
const SIDE: f32 = 18.;
/// The navigation row's top and height, the count row's gap above it and height, and the
/// divider's gap above it.
const NAV_TOP: f32 = 12.;
const ROW_HEIGHT: f32 = 40.;
const COUNT_TOP: f32 = 8.;
const COUNT_HEIGHT: f32 = 32.;
const DIVIDER_TOP: f32 = 13.;
const COUNT_TEXT: f32 = 14.;

impl MainWindow {
    /// The navigation row, the count row and the divider under them.
    pub(super) fn render_library_header(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let trail = Trail::of(self.library_panel.history.current(), super::crumbs::places(cx));
        let nav = div()
            .h(px(ROW_HEIGHT))
            .mt(px(NAV_TOP))
            .px(px(SIDE))
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .child(self.step_button(Step::Back, cx))
            .child(self.step_button(Step::Forward, cx))
            .child(self.render_crumbs(&trail, p, cx))
            .child(later("library-more", icons::MORE, tr!("More")))
            .child(pop_out_button(cx));
        div()
            .flex_row_start()
            .flex_col()
            .flex_none()
            .child(nav)
            .child(self.count_row(trail.outside_library, p, cx))
            .children(self.render_search_field(p, cx))
            .child(div().mx(px(INSET)).mt(px(DIVIDER_TOP)).h(px(1.)).bg(p.divider))
    }

    /// Back or Forward, enabled when the step leads somewhere.
    pub(super) fn step_button(&self, step: Step, cx: &mut Context<Self>) -> IconButton {
        let (id, icon, label, action): (_, _, _, &dyn Action) = match step {
            Step::Back => (
                "library-back",
                crate::i18n::mirrored(icons::ARROW_LEFT, icons::ARROW_RIGHT),
                tr!("Back"),
                &GoBack,
            ),
            Step::Forward => (
                "library-forward",
                crate::i18n::mirrored(icons::ARROW_RIGHT, icons::ARROW_LEFT),
                tr!("Forward"),
                &GoForward,
            ),
        };
        let button = IconButton::new(id, icon, label)
            .size(BUTTON, ICON)
            .disabled(!self.can_step_library(step, cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.step_library(step, cx)));
        with_shortcut(button, action, cx)
    }

    fn count_row(&self, outside_library: bool, p: &Palette, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(COUNT_HEIGHT))
            .mt(px(COUNT_TOP))
            .ps(px(INSET))
            .pe(px(SIDE))
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .child(self.count_text(outside_library, p, cx).flex_1())
            .child(self.library_sort_button(p, cx))
            .child(self.view_button(cx))
            .child(self.search_button(cx))
    }
}

impl MainWindow {
    /// "N items", with "Not in library" for a folder outside every media folder.
    pub(super) fn count_text(&self, outside_library: bool, p: &Palette, cx: &App) -> Div {
        let listed = self.listed_count(cx);
        let count = if outside_library {
            trn!("{count} item · Not in library", "{count} items · Not in library", listed)
        } else {
            item_count(listed)
        };
        div()
            .min_w_0()
            .truncate()
            .text_size(text_px(COUNT_TEXT))
            .text_color(p.text_muted)
            .child(SharedString::from(count))
    }

    /// View, showing the current view's icon, which opens the View menu.
    pub(super) fn view_button(&self, cx: &mut Context<Self>) -> Div {
        let button =
            IconButton::new("library-view", view_menu::icon(self.library_view(cx)), tr!("View"))
                .size(BUTTON, ICON)
                .disabled(*self.library_panel.history.current() == Route::Home)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle_view_menu(window, cx);
                }));
        div().relative().child(self.library_panel.view_button.element()).child(button)
    }
}

impl MainWindow {
    /// Search, shown on while the field is open, which opens and closes it.
    fn search_button(&self, cx: &mut Context<Self>) -> IconButton {
        IconButton::new("library-search", icons::SEARCH, tr!("Search"))
            .size(BUTTON, ICON)
            .selected(self.library_panel.search.is_some())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_library_search(window, cx);
            }))
    }
}

/// Pop-out: switches to full library mode.
fn pop_out_button(cx: &mut Context<MainWindow>) -> IconButton {
    IconButton::new("library-pop-out", icons::POP_OUT, tr!("Open full library"))
        .size(BUTTON, ICON)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
            this.open_full_library(window, cx);
        }))
}

/// A header button whose story hasn't shipped: shown, but disabled.
pub(super) fn later(id: &'static str, icon: &'static str, label: &'static str) -> IconButton {
    IconButton::new(id, icon, label).size(BUTTON, ICON).disabled(true)
}

fn with_shortcut(button: IconButton, action: &dyn Action, cx: &App) -> IconButton {
    match keymap::shortcut(action, cx) {
        Some(shortcut) => button.shortcut(shortcut),
        None => button,
    }
}
