//! The breadcrumb in the panel's header (S4.5.3): clickable segments with `▸` between them,
//! collapsed to `… ▸ parent ▸ current` from three levels deep, where `…` opens a menu of the
//! hidden segments. The last segment is where the library is, so it isn't a link.

use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Div, SharedString, Stateful, Window, div,
    prelude::*, px,
};

use super::header::GAP;
use super::{MainWindow, OpenMenu};
use crate::i18n::DirectionalStyle;
use crate::navigation::breadcrumb::{Crumb, Places, Trail};
use crate::playlists;
use crate::podcasts;
use crate::settings;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};
use crate::ui::{focus, metrics};

/// The segments' text size (`player_02`).
const TEXT: f32 = 16.;
/// The separator's size.
const SEPARATOR: f32 = 9.;

/// What the breadcrumb names folders and playlists after.
pub(super) fn places(cx: &App) -> Places<'_> {
    let local = &settings::get(cx).local;
    Places {
        video_roots: &local.media_folders,
        music_roots: &local.music_folders,
        playlists: playlists::summaries(cx),
        podcasts: podcasts::subscriptions(cx),
    }
}

impl MainWindow {
    /// The breadcrumb, taking the room between Forward and `•••`.
    pub(super) fn render_crumbs(&self, trail: &Trail, p: &Palette, cx: &mut Context<Self>) -> Div {
        let (hidden, shown) = trail.collapsed();
        let mut segments: Vec<AnyElement> = Vec::new();
        if !hidden.is_empty() {
            let hidden = hidden.to_vec();
            segments.push(self.render_more(hidden, p, cx).into_any_element());
        }
        let last = shown.len().saturating_sub(1);
        for (index, crumb) in shown.iter().enumerate() {
            if !segments.is_empty() {
                segments.push(separator(p).into_any_element());
            }
            let segment = if index == last {
                current(crumb, p).into_any_element()
            } else {
                link(index, crumb, p, cx).into_any_element()
            };
            segments.push(segment);
        }
        div()
            .flex_1()
            .min_w_0()
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .overflow_hidden()
            .text_size(text_px(TEXT))
            .children(segments)
    }

    /// `…`, which opens the menu of the hidden segments, nearest first.
    fn render_more(&self, hidden: Vec<Crumb>, p: &Palette, cx: &mut Context<Self>) -> Div {
        let element = segment(div().id("crumb-more"), p)
            .flex_none()
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.toggle_crumb_menu(hidden.clone(), window, cx);
            }))
            .child("…");
        div().relative().child(self.library_panel.crumb_button.element()).child(element)
    }

    fn toggle_crumb_menu(
        &mut self,
        hidden: Vec<Crumb>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.library_panel.crumb_menu.take().is_some() {
            cx.notify();
            return;
        }
        let main = cx.entity().downgrade();
        let items = hidden.into_iter().rev().map(|crumb| {
            let main = main.clone();
            MenuItem::action(crumb.label, move |_, cx| {
                let route = crumb.route.clone();
                main.update(cx, |main, cx| main.navigate(route, cx)).ok();
            })
        });
        let menu = ContextMenu::build(items.collect(), window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.library_panel.crumb_menu = None;
            cx.notify();
        });
        self.library_panel.crumb_menu = Some((menu, subscription));
        cx.notify();
    }
}

/// A segment that goes to its route.
fn link(index: usize, crumb: &Crumb, p: &Palette, cx: &mut Context<MainWindow>) -> Stateful<Div> {
    let route = crumb.route.clone();
    segment(div().id(("crumb", index)), p)
        .min_w_0()
        .truncate()
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.navigate(route.clone(), cx)))
        .child(SharedString::from(crumb.label.clone()))
}

/// The last segment: where the library is.
fn current(crumb: &Crumb, p: &Palette) -> Div {
    div()
        .min_w_0()
        .flex_shrink_0()
        .max_w_full()
        .truncate()
        .text_color(p.text_strong)
        .child(SharedString::from(crumb.label.clone()))
}

/// A clickable segment's look: muted, brighter on hover, ringed when focused.
fn segment(element: Stateful<Div>, p: &Palette) -> Stateful<Div> {
    let strong = p.text_strong;
    let element = element
        .px(px(2.))
        .rounded(px(metrics::RADIUS))
        .border_2()
        .border_color(gpui::transparent_black())
        .tab_index(0)
        .cursor_pointer()
        .text_color(p.text)
        .hover(move |style| style.text_color(strong));
    focus::ring(element, p)
}

/// The `▸` between segments.
fn separator(p: &Palette) -> Div {
    div().flex_none().text_size(text_px(SEPARATOR)).text_color(p.text_muted).child("▶")
}

/// The `…` menu below its segment, while open.
pub(super) fn render_menu(menu: &OpenMenu, button: &BoundsTracker) -> Option<AnyElement> {
    let (menu, _) = menu.as_ref()?;
    Some(popover::attached(button.bounds()?, Side::Below, 4., 1, menu.clone()))
}
