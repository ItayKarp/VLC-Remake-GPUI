//! Full library mode (S4.10.2, `player_03`–`07`): the library across the window under the title
//! bar, with no control bar. On the left the labeled navigation, each section's icon in the
//! rail's states ([`rail`](super::rail)) beside its name, then "Add item"; on the right one header
//! row (Back, Forward, the breadcrumb, the count, Sort, View, the search box and `•••`) over the
//! listing, laid out for the window's width ([`layout`](super::layout)). What plays shows in the
//! miniplayer card ([`miniplayer`](crate::main_window::miniplayer)).

use gpui::{ClickEvent, Context, Div, Hsla, Stateful, accesskit, div, prelude::*, px};

use super::header::{GAP, later};
use super::layout::{NAV_MARGIN, NAV_WIDTH};
use super::rail::section_icons;
use super::{INSET, MainWindow};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::navigation::breadcrumb::Trail;
use crate::navigation::history::Step;
use crate::navigation::rail::{self, RailItem, RailState};
use crate::navigation::route::Section;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::focus;
use crate::ui::icon::icon;
use crate::ui::metrics;

/// A navigation item's height, its spacing, its icon, the gap after the icon, and its text size
/// (`player_03`: items 46 px apart, the first 28 px below the top).
const NAV_ITEM: f32 = 40.;
const NAV_SPACING: f32 = 46.;
const NAV_TOP: f32 = 8.;
const NAV_ICON: f32 = 20.;
const NAV_ICON_GAP: f32 = 14.;
const NAV_PAD: f32 = 20.;
const NAV_TEXT: f32 = 14.;
/// The header row's height and its gap above, and the divider's gap above.
const HEADER_HEIGHT: f32 = 44.;
const HEADER_TOP: f32 = 16.;
const DIVIDER_TOP: f32 = 8.;

impl MainWindow {
    /// The labeled navigation and the listing under its header, across `root`.
    pub(super) fn render_full_library(
        &self,
        root: Stateful<Div>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let content = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex_row_start()
            .flex_col()
            .child(self.render_full_header(p, cx))
            .child(div().mx(px(INSET)).mt(px(DIVIDER_TOP)).h(px(1.)).bg(p.divider))
            .child(self.render_listing(p, cx));
        root.right_0().bg(p.background).child(self.render_labeled_nav(p, cx)).child(content)
    }

    /// Back, Forward, the breadcrumb, the count, Sort, View, the search box and `•••`.
    fn render_full_header(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let trail = Trail::of(self.library_panel.history.current(), super::crumbs::places(cx));
        div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .mt(px(HEADER_TOP))
            .px(px(INSET))
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .child(self.step_button(Step::Back, cx))
            .child(self.step_button(Step::Forward, cx))
            .child(self.render_crumbs(&trail, p, cx))
            .child(self.count_text(trail.outside_library, p, cx).flex_none())
            .child(self.library_sort_button(p, cx))
            .child(self.view_button(cx))
            .child(self.render_search_box(p, cx))
            .child(later("library-more", icons::MORE, tr!("More")))
    }

    /// The navigation down the left: each section, then "Add item".
    fn render_labeled_nav(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let route = self.library_panel.history.current();
        let items = RailItem::ALL.map(|item| match item {
            RailItem::Section(section) => {
                let item = nav_item(section, rail::state(item, route), p, cx);
                super::drag::spring_loaded(item, section, cx)
            }
            RailItem::Add => self.add_item(p, cx),
        });
        div()
            .flex_none()
            .w(px(NAV_WIDTH))
            .m(px(NAV_MARGIN))
            .pt(px(NAV_TOP))
            .flex_row_start()
            .flex_col()
            .gap(px(NAV_SPACING - NAV_ITEM))
            .rounded(px(metrics::RADIUS_LARGE))
            .bg(p.sidebar)
            .children(items)
    }

    /// "Add item", which opens the rail's "+" menu beside it (S4.5.7).
    fn add_item(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let open = self.library_panel.add_menu.is_some();
        let item = item_frame("nav-add", tr!("Add item"), p)
            .aria_expanded(open)
            .when(open, |item| item.bg(p.hover))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_add_menu(window, cx);
            }))
            .child(icon(icons::ADD, NAV_ICON, p.icon))
            .child(tr!("Add item"));
        div().relative().child(self.library_panel.add_button.element()).child(item)
    }
}

/// A section's item: its icon in its rail state, and its name; a click goes to its root.
fn nav_item(section: Section, state: RailState, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let (outline, filled) = section_icons(section);
    let (glyph, color, text): (_, Hsla, Hsla) = match state {
        RailState::Outline => (outline, p.icon, p.text),
        RailState::Root => (filled, p.accent_fg, p.text_strong),
        RailState::Inside => (filled, p.icon, p.text_strong),
    };
    let item = item_frame(("nav", section as usize), section.label(), p)
        .aria_selected(state == RailState::Root)
        .text_color(text)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.navigate(section.root(), cx);
        }))
        .child(icon(glyph, NAV_ICON, color))
        .child(section.label());
    div().child(item)
}

/// A navigation item's frame: its height, padding, hover and text, and a tab stop named
/// `label` that Enter and Space press (S9.1.1).
fn item_frame(id: impl Into<gpui::ElementId>, label: &'static str, p: &Palette) -> Stateful<Div> {
    let hover = p.hover;
    let frame = div()
        .id(id)
        .tab_index(0)
        .key_context(focus::CONTROL)
        .role(accesskit::Role::Button)
        .aria_label(label);
    focus::outline_ring(frame, p)
        .h(px(NAV_ITEM))
        .mx(px(NAV_MARGIN))
        .px(px(NAV_PAD - NAV_MARGIN))
        .flex_row_start()
        .items_center()
        .gap(px(NAV_ICON_GAP))
        .rounded(px(metrics::RADIUS))
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .text_size(text_px(NAV_TEXT))
        .text_color(p.text)
}
