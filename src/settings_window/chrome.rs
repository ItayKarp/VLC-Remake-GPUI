//! The parts of the Settings window around the page (`docs/ui/settings/screens/*`): the top row
//! with the search field over the sidebar and the page title with a close ✕ over the page, and
//! the empty state a search that finds nothing shows in place of the page.
//!
//! The empty state isn't in the designs: a search icon, "No settings match “xyz”", a hint, and
//! **Clear Search**.

use gpui::{AnyElement, Context, Div, Entity, Stateful, WindowControlArea, div, prelude::*, px};

use super::SettingsWindow;
use super::keys::SEARCH_CONTEXT;
use crate::assets::icons;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::Button;
use crate::ui::icon::icon;
use crate::ui::text_input::TextInput;
use crate::ui::{focus, metrics};
use crate::window_chrome::{self, TITLE_BAR_HEIGHT};

/// The gutters around the sidebar and the page.
pub(super) const MARGIN: f32 = 8.;
/// The top row's height, and the search field's.
pub(super) const TOP_ROW_HEIGHT: f32 = 40.;
const SEARCH_HEIGHT: f32 = 32.;
/// The page title's size, and its inset after the search field, which lines it up with the
/// page's rows (the gutter, then the page's padding).
const TITLE_TEXT: f32 = 18.;
const TITLE_INSET: f32 = MARGIN + 30.;
/// The close button and its icon.
const CLOSE_SIZE: f32 = 32.;
const CLOSE_ICON: f32 = 20.;
/// The empty state's icon and heading size, and the space between its lines.
const EMPTY_ICON: f32 = 40.;
const EMPTY_HEADING: f32 = 18.;
const EMPTY_GAP: f32 = 12.;
/// How wide the empty state's text runs before it wraps.
const EMPTY_WIDTH: f32 = 360.;

/// The strip at the top where macOS draws the traffic lights; nothing elsewhere.
pub(super) fn traffic_light_strip() -> Option<Stateful<Div>> {
    cfg!(target_os = "macos").then(|| {
        window_chrome::drags_window(div().id("settings-traffic-lights"))
            .flex_none()
            .h(px(TITLE_BAR_HEIGHT))
    })
}

/// The top row: the search field (`search_width` wide), then the page title and ✕. Dragging it
/// moves the window.
pub(super) fn top_row(
    search: Div,
    search_width: f32,
    title: &'static str,
    p: &Palette,
) -> Stateful<Div> {
    let title = div()
        .flex_1()
        .min_w_0()
        .pl(px(TITLE_INSET))
        .truncate()
        .text_size(text_px(TITLE_TEXT))
        .text_color(p.text_strong)
        .child(title);
    let row = div()
        .id("settings-top-row")
        .flex_none()
        .h(px(TOP_ROW_HEIGHT))
        .flex()
        .items_center()
        .px(px(MARGIN))
        .window_control_area(WindowControlArea::Drag)
        .child(div().w(px(search_width)).flex_none().occlude().child(search))
        .child(title)
        .child(close_button(p));
    window_chrome::drags_window(row)
}

/// The search field: a raised face over the dark top row, with a search icon and the query.
pub(super) fn search_field(query: &Entity<TextInput>, p: &Palette) -> Div {
    div()
        .key_context(SEARCH_CONTEXT)
        .h(px(SEARCH_HEIGHT))
        .flex()
        .items_center()
        .gap(px(10.))
        .px(px(12.))
        .rounded(px(metrics::RADIUS))
        .bg(p.control)
        .text_size(text_px(metrics::TEXT))
        .child(icon(icons::SEARCH, metrics::ICON, p.icon))
        .child(div().flex_1().min_w_0().child(query.clone()))
}

/// ✕: closes the window.
fn close_button(p: &Palette) -> Stateful<Div> {
    let hover = p.button_hover;
    let button = div()
        .id("close-settings")
        .flex_none()
        .size(px(CLOSE_SIZE))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(metrics::RADIUS))
        .border_2()
        .border_color(gpui::transparent_black())
        .occlude()
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .tab_index(0)
        .aria_label("Close Settings")
        .on_click(|_, window, _| window.remove_window())
        .child(icon(icons::CLOSE, CLOSE_ICON, p.icon));
    focus::ring(button, p)
}

/// What shows in place of the page when the search finds nothing anywhere.
pub(super) fn no_matches(query: &str, p: &Palette, cx: &mut Context<SettingsWindow>) -> AnyElement {
    let clear = Button::new("clear-search", "Clear Search")
        .on_click(cx.listener(|this, _, window, cx| this.clear_search(window, cx)));
    div()
        .id("settings-no-matches")
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(EMPTY_GAP))
        .text_center()
        .child(icon(icons::SEARCH, EMPTY_ICON, p.text_muted))
        .child(
            div()
                .max_w(px(EMPTY_WIDTH))
                .text_size(text_px(EMPTY_HEADING))
                .text_color(p.text_strong)
                .child(format!("No settings match \u{201c}{query}\u{201d}")),
        )
        .child(
            div()
                .max_w(px(EMPTY_WIDTH))
                .text_size(text_px(metrics::TEXT))
                .text_color(p.text_muted)
                .child("Check the spelling, or try fewer or different words."),
        )
        .child(clear)
        .into_any_element()
}
