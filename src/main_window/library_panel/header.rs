//! The panel's header (`library/sidebar/*`): Back and Forward with their real enabled states
//! (S4.5.2), the breadcrumb, `•••` and pop-out; under them the item count (S4.5.5), with "Not in
//! library" for a folder outside every media folder, and Sort, View and Search; then a divider.
//!
//! View opens its menu ([`view_menu`](super::view_menu)). `•••` (its menu isn't designed),
//! pop-out (full library mode, S4.10.2), Sort and Search (S4.7) come with their stories and show
//! disabled until then.

use gpui::{Action, App, ClickEvent, Context, Div, SharedString, div, prelude::*, px};

use super::INSET;
use super::MainWindow;
use super::view_menu;
use crate::assets::icons;
use crate::keymap;
use crate::keymap::actions::{GoBack, GoForward};
use crate::navigation::breadcrumb::Trail;
use crate::navigation::history::Step;
use crate::navigation::item_count;
use crate::theme::Palette;
use crate::ui::button::IconButton;

/// The header's buttons: 28 px, 8 px apart, 18 px in from the panel's sides (`player_02`).
const BUTTON: f32 = 28.;
const ICON: f32 = 20.;
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
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(self.step_button(Step::Back, cx))
            .child(self.step_button(Step::Forward, cx))
            .child(self.render_crumbs(&trail, p, cx))
            .child(later("library-more", icons::MORE, "More"))
            .child(later("library-pop-out", icons::POP_OUT, "Open full library"));
        div()
            .flex()
            .flex_col()
            .flex_none()
            .child(nav)
            .child(self.count_row(trail.outside_library, p, cx))
            .child(div().mx(px(INSET)).mt(px(DIVIDER_TOP)).h(px(1.)).bg(p.divider))
    }

    fn step_button(&self, step: Step, cx: &mut Context<Self>) -> IconButton {
        let (id, icon, label, action): (_, _, _, &dyn Action) = match step {
            Step::Back => ("library-back", icons::ARROW_LEFT, "Back", &GoBack),
            Step::Forward => ("library-forward", icons::ARROW_RIGHT, "Forward", &GoForward),
        };
        let button = IconButton::new(id, icon, label)
            .size(BUTTON, ICON)
            .disabled(!self.can_step_library(step, cx))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.step_library(step, cx)));
        with_shortcut(button, action, cx)
    }

    fn count_row(&self, outside_library: bool, p: &Palette, cx: &mut Context<Self>) -> Div {
        let mut count = item_count(self.listed_count(cx));
        if outside_library {
            count.push_str(" · Not in library");
        }
        div()
            .h(px(COUNT_HEIGHT))
            .mt(px(COUNT_TOP))
            .pl(px(INSET))
            .pr(px(SIDE))
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(px(COUNT_TEXT))
                    .text_color(p.text_muted)
                    .child(SharedString::from(count)),
            )
            .child(later("library-sort", icons::SORT, "Sort"))
            .child(self.view_button(cx))
            .child(later("library-search", icons::SEARCH, "Search"))
    }
}

impl MainWindow {
    /// View, showing the current view's icon, which opens the View menu.
    fn view_button(&self, cx: &mut Context<Self>) -> Div {
        let button =
            IconButton::new("library-view", view_menu::icon(self.library_view(cx)), "View")
                .size(BUTTON, ICON)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle_view_menu(window, cx);
                }));
        div().relative().child(self.library_panel.view_button.element()).child(button)
    }
}

/// A header button whose story hasn't shipped: shown, but disabled.
fn later(id: &'static str, icon: &'static str, label: &'static str) -> IconButton {
    IconButton::new(id, icon, label).size(BUTTON, ICON).disabled(true)
}

fn with_shortcut(button: IconButton, action: &dyn Action, cx: &App) -> IconButton {
    match keymap::shortcut(action, cx) {
        Some(shortcut) => button.shortcut(shortcut),
        None => button,
    }
}
