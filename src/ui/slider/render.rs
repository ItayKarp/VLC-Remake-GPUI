//! Drawing a [`Slider`]: track, fill, zero tick, thumb, drag tooltip and the
//! optional numeric field.

use gpui::{
    AnyElement, Context, Div, Entity, MouseButton, MouseMoveEvent, MouseUpEvent, SharedString,
    Stateful, Window, accesskit, canvas, div, prelude::*, px, relative,
};

use super::{Orientation, Slider, Tone};
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::{focus, metrics};

/// Thickness of the track.
const TRACK: f32 = 12.;
/// The thumb's length across the track.
const THUMB_LONG: f32 = 22.;
/// The thumb's width along the track.
const THUMB_SHORT: f32 = 8.;
/// Shortest the slider gets along its axis.
const MIN_LENGTH: f32 = 40.;
/// Width of the numeric field next to the slider.
const FIELD_WIDTH: f32 = 96.;
/// The transparent border the focus ring draws in.
const FOCUS_BORDER: f32 = 2.;
/// A disabled slider's opacity, as the kit's other disabled controls.
const DISABLED_OPACITY: f32 = 0.4;

impl Render for Slider {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let slider = focus::ring(self.slider_element(&p, cx), &p);
        // The labels line up with the track: inside the thumb's room and the focus border.
        let labels = self.mark_labels(THUMB_SHORT / 2. + FOCUS_BORDER, &p);
        let slider = match labels {
            // A block, so the slider and its labels stack and fill its width.
            Some(labels) => {
                div().flex_1().min_w(px(MIN_LENGTH)).child(slider).child(labels).into_any_element()
            }
            None => slider.into_any_element(),
        };
        match &self.field {
            Some((field, _)) => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .w_full()
                .child(slider)
                .child(div().w(px(FIELD_WIDTH)).flex_none().child(field.clone()))
                .into_any_element(),
            None => slider.into_any_element(),
        }
    }
}

impl Slider {
    pub(super) fn horizontal(&self) -> bool {
        self.orientation == Orientation::Horizontal
    }

    fn value_text(&self) -> SharedString {
        (self.format)(self.model.value()).into()
    }

    /// The focusable control: the track plus, while dragging, the window-wide
    /// mouse listener.
    fn slider_element(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let horizontal = self.horizontal();
        let slider = div()
            .id("slider")
            .flex()
            .items_center()
            .justify_center()
            .map(|s| {
                if horizontal {
                    s.flex_1().min_w(px(MIN_LENGTH)).h(px(THUMB_LONG + 6.)).px(px(THUMB_SHORT / 2.))
                } else {
                    s.h_full().min_h(px(MIN_LENGTH)).w(px(THUMB_LONG + 6.)).py(px(THUMB_SHORT / 2.))
                }
            })
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .key_context(focus::CONTROL)
            .track_focus(&self.focus_handle)
            .map(|s| {
                if self.is_disabled() {
                    s.opacity(DISABLED_OPACITY)
                } else {
                    s.cursor_pointer()
                        .on_key_down(cx.listener(Self::on_key_down))
                        .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                }
            })
            .child(self.track_element(p))
            .children(self.drag_listener(cx));
        self.with_accessibility(slider)
    }

    /// Describes the slider to screen readers.
    fn with_accessibility(&self, slider: Stateful<Div>) -> Stateful<Div> {
        slider
            .role(accesskit::Role::Slider)
            .aria_label(self.label.clone())
            .aria_numeric_value(self.model.value())
            .aria_min_numeric_value(self.model.min)
            .aria_max_numeric_value(self.model.max)
            .aria_numeric_value_step(self.model.step)
            .aria_value(self.value_text())
            .aria_orientation(if self.horizontal() {
                accesskit::Orientation::Horizontal
            } else {
                accesskit::Orientation::Vertical
            })
    }

    fn track_element(&self, p: &Palette) -> Div {
        let horizontal = self.horizontal();
        div()
            .relative()
            .rounded(px(TRACK / 2.))
            .bg(p.well)
            .map(|t| if horizontal { t.h(px(TRACK)).w_full() } else { t.w(px(TRACK)).h_full() })
            .child(self.track.element())
            .child(self.fill_element(p))
            .children(self.ticks(p))
            .children(self.zero_tick(p))
            .child(self.thumb(p))
    }

    /// The fill and thumb colors for the slider's tone.
    fn colors(&self, p: &Palette) -> (gpui::Hsla, gpui::Hsla) {
        match self.tone {
            Tone::Accent => (p.accent_tint, p.accent_strong),
            Tone::Neutral => (p.slider_fill, p.slider_thumb),
        }
    }

    fn fill_element(&self, p: &Palette) -> Div {
        let horizontal = self.horizontal();
        let (fill_start, fill_end) = self.model.fill(self.centered);
        let (start, len) = (fill_start as f32, (fill_end - fill_start) as f32);
        div().absolute().bg(self.colors(p).0).rounded(px(TRACK / 2.)).map(|f| {
            if horizontal {
                f.top_0().bottom_0().left(relative(start)).w(relative(len))
            } else {
                f.left_0().right_0().bottom(relative(start)).h(relative(len))
            }
        })
    }

    /// A tick at zero on centered sliders (`video_colors`, the delay slider).
    fn zero_tick(&self, p: &Palette) -> Option<Div> {
        if !self.centered {
            return None;
        }
        let zero = self.model.zero_fraction() as f32;
        let tick = div().absolute().bg(p.divider);
        Some(if self.horizontal() {
            tick.top(px(3.)).bottom(px(3.)).w(px(2.)).left(relative(zero))
        } else {
            tick.left(px(3.)).right(px(3.)).h(px(2.)).bottom(relative(zero))
        })
    }

    fn thumb(&self, p: &Palette) -> Div {
        let horizontal = self.horizontal();
        let fraction = self.model.fraction() as f32;
        div()
            .absolute()
            .rounded(px(3.))
            .bg(self.colors(p).1)
            .map(|t| {
                if horizontal {
                    t.w(px(THUMB_SHORT))
                        .h(px(THUMB_LONG))
                        .top(px((TRACK - THUMB_LONG) / 2.))
                        .left(relative(fraction))
                        .ml(px(-THUMB_SHORT / 2.))
                } else {
                    t.h(px(THUMB_SHORT))
                        .w(px(THUMB_LONG))
                        .left(px((TRACK - THUMB_LONG) / 2.))
                        .bottom(relative(fraction))
                        .mb(px(-THUMB_SHORT / 2.))
                }
            })
            .when(self.is_dragging(), |t| t.child(self.value_tooltip(p)))
    }

    /// The value tooltip shown above the thumb while dragging.
    fn value_tooltip(&self, p: &Palette) -> Div {
        let bubble = div()
            .px(px(6.))
            .py(px(2.))
            .rounded(px(4.))
            .bg(p.tooltip)
            .text_size(text_px(12.))
            .text_color(p.tooltip_text)
            .whitespace_nowrap()
            .child(self.value_text());
        div()
            .absolute()
            .bottom(px(THUMB_LONG + 6.))
            .left(px(-30.))
            .w(px(60. + THUMB_SHORT))
            .flex()
            .justify_center()
            .child(bubble)
    }

    /// While dragging, follows the mouse anywhere in the window.
    fn drag_listener(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.is_dragging() {
            return None;
        }
        let entity = cx.entity();
        let listener =
            canvas(|_, _, _| {}, move |_, (), window, _| listen_for_drag(entity, window));
        Some(listener.absolute().size_0().into_any_element())
    }
}

/// Sends every mouse move and the release to `slider`, wherever the mouse is.
fn listen_for_drag(slider: Entity<Slider>, window: &mut Window) {
    let moved = slider.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
        if phase.bubble() && event.dragging() {
            moved.update(cx, |this, cx| this.drag_to(event.position, window, cx));
        }
    });
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
        if phase.bubble() {
            slider.update(cx, Slider::end_drag);
        }
    });
}
