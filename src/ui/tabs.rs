//! Tabs with icon + label and an accent underline (S0.4.4b), as in the in-player
//! panels (`player/controls/control_bar/controls-*_settings`: Playback, Video,
//! Audio, Subtitles). One tab stop; Left/Right switch tabs.

use std::rc::Rc;

use gpui::{
    App, ElementId, KeyDownEvent, SharedString, Stateful, Window, accesskit, div, prelude::*, px,
};

use super::focus::{self, Axis, Layout};
use super::icon::icon;
use crate::i18n::DirectionalStyle;
use crate::theme::text_px;
use crate::theme::{self, Palette};

/// Called with the index of the tab the user picked.
type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

/// The height of the tab bar.
pub const TAB_HEIGHT: f32 = 48.;

/// One tab: an optional icon and a label.
pub struct Tab {
    /// The icon's asset path, drawn before the label.
    pub icon: Option<&'static str>,
    /// The tab's name.
    pub label: SharedString,
}

impl Tab {
    /// A tab with an optional icon and a label.
    pub fn new(icon: Option<&'static str>, label: impl Into<SharedString>) -> Self {
        Self { icon, label: label.into() }
    }
}

/// A row of tabs with one selected; the owner keeps the selection and changes it from
/// [`Self::on_select`].
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    items: Vec<Tab>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl Tabs {
    /// A tab bar showing `tabs`, with the tab at index `selected` selected.
    pub fn new(id: impl Into<ElementId>, tabs: Vec<Tab>, selected: usize) -> Self {
        Self { id: id.into(), items: tabs, selected, on_select: None }
    }

    /// Called when the user clicks another tab or moves to one with the arrow keys.
    pub fn on_select(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let selected = self.selected;
        let len = self.items.len();
        let on_select = self.on_select;
        let tabs = self.items.into_iter().enumerate().map(|(i, tab)| {
            render_tab(
                TabState { index: i, is_selected: i == selected },
                tab,
                &p,
                on_select.clone(),
            )
        });

        let bar = div()
            .id(self.id)
            .h(px(TAB_HEIGHT))
            .flex_row_start()
            .items_end()
            .gap(px(4.))
            .px(px(8.))
            .border_2()
            .border_color(gpui::transparent_black())
            .rounded(px(4.))
            .key_context(focus::CONTROL)
            .tab_index(0)
            .role(accesskit::Role::TabList)
            .children(tabs)
            .when_some(on_select, |bar, f| bar.on_key_down(key_handler(selected, len, f)));
        focus::ring(bar, &p)
    }
}

/// Where a tab sits in the bar.
#[derive(Clone, Copy)]
struct TabState {
    index: usize,
    is_selected: bool,
}

/// One tab: icon, label and the accent underline when selected.
fn render_tab(
    state: TabState,
    tab: Tab,
    p: &Palette,
    on_select: Option<SelectHandler>,
) -> Stateful<gpui::Div> {
    let TabState { index, is_selected } = state;
    let hover = p.text;
    let icon_color = if is_selected { p.accent_fg } else { p.icon };
    div()
        .id(index)
        .h_full()
        .flex_row_start()
        .items_center()
        .gap(px(6.))
        .px(px(10.))
        .border_b_2()
        .border_color(if is_selected { p.accent_fg } else { gpui::transparent_black() })
        .text_size(text_px(13.))
        .text_color(if is_selected { p.text_strong } else { p.text_muted })
        .role(accesskit::Role::Tab)
        .aria_selected(is_selected)
        .when(!is_selected, |t| t.cursor_pointer().hover(move |s| s.text_color(hover)))
        .when_some(on_select, |t, f| {
            t.on_click(move |_, window, cx| {
                if !is_selected {
                    f(&index, window, cx);
                }
            })
        })
        .when_some(tab.icon, |t, path| t.child(icon(path, 16., icon_color)))
        .child(div().whitespace_nowrap().child(tab.label))
}

/// The tab bar's key handler: arrow keys, Home and End select another tab.
fn key_handler(
    selected: usize,
    len: usize,
    on_select: SelectHandler,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        let Some(next) = arrow_target(event, selected, len) else {
            return;
        };
        cx.stop_propagation();
        if next != selected {
            on_select(&next, window, cx);
        }
    }
}

/// The tab an arrow, Home or End key moves the selection to, wrapping around the ends.
/// `None` for keys the tab bar doesn't handle.
fn arrow_target(event: &KeyDownEvent, selected: usize, len: usize) -> Option<usize> {
    let movement = focus::group_key(event, Axis::Horizontal)?;
    focus::step(Some(selected), len, movement, Layout::WRAPPING, |_| true)
}
