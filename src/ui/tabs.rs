//! Tabs with icon + label and an accent underline (S0.4.4b), as in the in-player
//! panels (`player/controls/control_bar/controls-*_settings`: Playback, Video,
//! Audio, Subtitles). One tab stop; Left/Right switch tabs.

use std::rc::Rc;

use gpui::{App, ElementId, SharedString, Window, accesskit, div, prelude::*, px};

use super::focus::{self, Axis, Layout};
use super::icon::icon;
use crate::theme;

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

pub const TAB_HEIGHT: f32 = 48.;

pub struct Tab {
    pub icon: Option<&'static str>,
    pub label: SharedString,
}

impl Tab {
    pub fn new(icon: Option<&'static str>, label: impl Into<SharedString>) -> Self {
        Self {
            icon,
            label: label.into(),
        }
    }
}

#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    tabs: Vec<Tab>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>, tabs: Vec<Tab>, selected: usize) -> Self {
        Self {
            id: id.into(),
            tabs,
            selected,
            on_select: None,
        }
    }

    pub fn on_select(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let selected = self.selected;
        let len = self.tabs.len();
        let on_select = self.on_select;

        let tabs = self.tabs.into_iter().enumerate().map(|(i, tab)| {
            let is_selected = i == selected;
            let on_select = on_select.clone();
            let hover = p.text;
            div()
                .id(i)
                .h_full()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .px(px(10.))
                .border_b_2()
                .border_color(if is_selected {
                    p.accent_fg
                } else {
                    gpui::transparent_black()
                })
                .text_size(px(13.))
                .text_color(if is_selected {
                    p.text_strong
                } else {
                    p.text_muted
                })
                .role(accesskit::Role::Tab)
                .aria_selected(is_selected)
                .when(!is_selected, |t| {
                    t.cursor_pointer().hover(move |s| s.text_color(hover))
                })
                .when_some(on_select, |t, f| {
                    t.on_click(move |_, window, cx| {
                        if !is_selected {
                            f(&i, window, cx)
                        }
                    })
                })
                .when_some(tab.icon, |t, path| {
                    t.child(icon(
                        path,
                        16.,
                        if is_selected { p.accent_fg } else { p.icon },
                    ))
                })
                .child(div().whitespace_nowrap().child(tab.label))
        });

        let bar = div()
            .id(self.id)
            .h(px(TAB_HEIGHT))
            .flex()
            .flex_row()
            .items_end()
            .gap(px(4.))
            .px(px(8.))
            .border_2()
            .border_color(gpui::transparent_black())
            .rounded(px(4.))
            .tab_index(0)
            .role(accesskit::Role::TabList)
            .children(tabs)
            .when_some(on_select, |bar, f| {
                bar.on_key_down(move |event, window, cx| {
                    let Some(movement) = focus::group_key(event, Axis::Horizontal) else {
                        return;
                    };
                    if let Some(next) =
                        focus::step(Some(selected), len, movement, Layout::WRAPPING, |_| true)
                    {
                        cx.stop_propagation();
                        if next != selected {
                            f(&next, window, cx);
                        }
                    }
                })
            });
        focus::ring(bar, &p)
    }
}
