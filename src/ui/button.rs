//! Buttons (S0.4.1) and icon buttons (S0.4.2).
//!
//! `shared/components/button-default` is a well-colored face (`#090909`) with
//! muted text and an optional leading icon; `button-variant2` is the same on the
//! raised control color (`#191919`). Primary buttons are accent filled, as in
//! onboarding. Every style has hover, pressed, keyboard focus and disabled states.
//!
//! GPUI clicks a focused element on Enter or Space, so buttons need no key
//! handling of their own.

use std::rc::Rc;

use gpui::{
    App, ClickEvent, ElementId, FocusHandle, FontWeight, Hsla, MouseButton, SharedString, Window,
    div, prelude::*, px,
};

use super::icon::icon;
use super::{focus, metrics, tooltip};
use crate::theme::{self, Palette, blend};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ButtonStyle {
    /// `button-default`: the well color, for secondary actions in panels.
    #[default]
    Subtle,
    /// `button-variant2`: the raised control color (onboarding's Back, Browse).
    Filled,
    /// Accent filled: the default action of a screen or dialog.
    Primary,
    /// No background until hovered: toolbar-like actions.
    Ghost,
}

struct Colors {
    bg: Hsla,
    hover: Hsla,
    pressed: Hsla,
    fg: Hsla,
}

impl ButtonStyle {
    fn colors(self, p: &Palette) -> Colors {
        match self {
            ButtonStyle::Subtle => {
                Colors { bg: p.well, hover: p.control, pressed: p.hover, fg: p.text_muted }
            }
            ButtonStyle::Filled => Colors {
                bg: p.control,
                hover: p.button_hover,
                pressed: p.selected,
                fg: p.text_muted,
            },
            ButtonStyle::Primary => Colors {
                bg: p.accent,
                hover: blend(p.accent, p.background, 0.85),
                pressed: blend(p.accent, p.background, 0.7),
                fg: p.on_accent,
            },
            ButtonStyle::Ghost => Colors {
                bg: gpui::transparent_black(),
                hover: p.hover,
                pressed: p.selected,
                fg: p.text,
            },
        }
    }
}

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    icon: Option<&'static str>,
    style: ButtonStyle,
    disabled: bool,
    width: Option<f32>,
    tooltip: Option<(SharedString, Option<SharedString>)>,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            style: ButtonStyle::default(),
            disabled: false,
            width: None,
            tooltip: None,
            focus: None,
            on_click: None,
        }
    }

    /// Uses `handle` for focus instead of an anonymous one, so the owner can tell
    /// when this button has focus (dialogs do, for Enter).
    pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
        self.focus = Some(handle.clone());
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn primary(self) -> Self {
        self.style(ButtonStyle::Primary)
    }

    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn tooltip(
        mut self,
        label: impl Into<SharedString>,
        shortcut: Option<impl Into<SharedString>>,
    ) -> Self {
        self.tooltip = Some((label.into(), shortcut.map(Into::into)));
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let colors = self.style.colors(&p);
        let (hover, pressed) = (colors.hover, colors.pressed);
        let enabled = !self.disabled;

        div()
            .id(self.id)
            .h(px(metrics::BUTTON_HEIGHT))
            .px(px(16.))
            .when_some(self.width, |b, w| b.w(px(w)))
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .rounded(px(metrics::RADIUS))
            .bg(colors.bg)
            .border_2()
            .border_color(gpui::transparent_black())
            .text_size(px(metrics::TEXT))
            .font_weight(if self.style == ButtonStyle::Primary {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .text_color(colors.fg)
            .overflow_hidden()
            .aria_label(self.label.clone())
            .when(enabled, |b| {
                let b = match &self.focus {
                    Some(handle) => b.track_focus(handle),
                    None => b.tab_index(0),
                };
                let b =
                    b.cursor_pointer().hover(move |s| s.bg(hover)).active(move |s| s.bg(pressed));
                focus::ring(b, &p)
            })
            .when(!enabled, |b| b.opacity(0.4))
            .when_some(self.icon, |b, path| b.child(icon(path, metrics::ICON, colors.fg)))
            .child(div().truncate().child(self.label))
            .when_some(self.tooltip.filter(|_| enabled), |b, (label, shortcut)| {
                b.tooltip(tooltip::text(label, shortcut))
            })
            .when_some(self.on_click.filter(|_| enabled), |b, f| {
                b.on_click(move |event, window, cx| f(event, window, cx))
            })
    }
}

/// A square icon-only button with a tooltip that names it and its shortcut.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: &'static str,
    label: SharedString,
    shortcut: Option<SharedString>,
    size: f32,
    icon_size: f32,
    selected: bool,
    disabled: bool,
    /// Stop mouse-down from reaching a window drag area underneath.
    in_title_bar: bool,
    on_click: Option<ClickHandler>,
}

impl IconButton {
    pub fn new(
        id: impl Into<ElementId>,
        icon: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            icon,
            label: label.into(),
            shortcut: None,
            size: 28.,
            icon_size: 18.,
            selected: false,
            disabled: false,
            in_title_bar: false,
            on_click: None,
        }
    }

    pub fn shortcut(mut self, shortcut: impl Into<SharedString>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    pub fn size(mut self, size: f32, icon_size: f32) -> Self {
        self.size = size;
        self.icon_size = icon_size;
        self
    }

    /// Shows the button as on (an open popover, an active panel).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn in_title_bar(mut self) -> Self {
        self.in_title_bar = true;
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let (hover, pressed) = (p.button_hover, p.selected);
        let enabled = !self.disabled;
        let color = if self.selected { p.accent_fg } else { p.icon };
        let name = tooltip::describe(&self.label, self.shortcut.as_deref());

        div()
            .id(self.id)
            .size(px(self.size))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .occlude()
            .aria_label(name)
            .when(self.selected, |b| b.bg(hover))
            .when(enabled, |b| {
                let b = b
                    .tab_index(0)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover))
                    .active(move |s| s.bg(pressed))
                    .tooltip(tooltip::text(self.label.clone(), self.shortcut.clone()));
                focus::ring(b, &p)
            })
            .when(!enabled, |b| b.opacity(0.4))
            .when(self.in_title_bar, |b| {
                b.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            })
            .when_some(self.on_click.filter(|_| enabled), |b, f| {
                b.on_click(move |event, window, cx| f(event, window, cx))
            })
            .child(icon(self.icon, self.icon_size, color))
    }
}
