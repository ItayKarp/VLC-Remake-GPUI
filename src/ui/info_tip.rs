//! The `(i)` icon (S5.2, S9.1.1): hovering shows its text as a tooltip, and so does keyboard
//! focus. Tooltips are hover-only in GPUI, so with focus the icon draws the same bubble
//! itself, under the icon. Screen readers get the text as the icon's description.

use gpui::{
    App, ElementId, FocusHandle, SharedString, Window, accesskit, anchored, deferred, div,
    prelude::*, px,
};

use super::focus;
use super::icon::icon;
use super::tooltip;
use crate::assets::icons;
use crate::theme;
use crate::theme::text_px;

/// How far under the icon the bubble sits.
const BUBBLE_GAP: f32 = 6.;
/// The widest the bubble gets before its text wraps.
const BUBBLE_WIDTH: f32 = 320.;

/// An `(i)` icon explaining `about` with `text`.
#[derive(IntoElement)]
pub struct InfoTip {
    id: ElementId,
    about: SharedString,
    text: SharedString,
    size: f32,
}

impl InfoTip {
    /// The icon at `size`, explaining the setting named `about` with `text`.
    pub fn new(
        id: impl Into<ElementId>,
        about: impl Into<SharedString>,
        text: impl Into<SharedString>,
        size: f32,
    ) -> Self {
        Self { id: id.into(), about: about.into(), text: text.into(), size }
    }
}

impl RenderOnce for InfoTip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let handle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let bubble = shows_bubble(&handle, window).then(|| bubble(self.text.clone(), window, cx));
        let tip = div()
            .id(self.id)
            .flex_none()
            .track_focus(&handle)
            .key_context(focus::CONTROL)
            .rounded(px(2.))
            .role(accesskit::Role::Button)
            .aria_label(SharedString::from(format!("About {}", self.about)))
            .aria_description(self.text.clone())
            .tooltip(tooltip::text(self.text, None::<SharedString>))
            .child(icon(icons::INFO, self.size, p.icon))
            .children(bubble);
        focus::outline_ring(tip, &p)
    }
}

/// Whether the icon has the keyboard's focus, which shows its text.
fn shows_bubble(handle: &FocusHandle, window: &Window) -> bool {
    handle.is_focused(window) && window.last_input_was_keyboard()
}

/// The tooltip's bubble, under the icon and over everything else.
fn bubble(text: SharedString, window: &Window, cx: &App) -> impl IntoElement {
    let p = theme::palette(window, cx);
    deferred(
        anchored().child(
            div()
                .mt(px(BUBBLE_GAP))
                .max_w(px(BUBBLE_WIDTH))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(4.))
                .bg(p.tooltip)
                .border_1()
                .border_color(p.divider)
                .shadow_md()
                .text_size(text_px(12.))
                .line_height(text_px(16.))
                .text_color(p.tooltip_text)
                .child(text),
        ),
    )
}
