//! Sliders (S0.4.5): `player/panels/video/colors/video_colors-expanded`
//! (Saturation, Sepia Intensity), the audio delay slider and the EQ bands.
//!
//! Horizontal or vertical; a neutral or accent fill; filled from the minimum or
//! from zero ("centered", for balance, delay and EQ gain). Drag the thumb or click
//! the track to jump; arrow keys step (Shift for a big step), Home/End/PageUp/
//! PageDown work too; double-click resets to the default. While dragging, a
//! tooltip shows the value. An optional numeric field edits it directly.
//!
//! Values from settings or the engine are clamped for display, and NaN/infinite
//! values are ignored. Held keys auto-repeat, but [`Changed`] is throttled so the
//! engine isn't flooded; the final value always arrives.

use std::time::{Duration, Instant};

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, SharedString, Subscription, Window, accesskit,
    canvas, div, prelude::*, px, relative,
};

use super::popover::BoundsTracker;
use super::text_input::{self, TextInput};
use super::{focus, metrics};
use crate::theme;

const TRACK: f32 = 12.;
const THUMB_LONG: f32 = 22.;
const THUMB_SHORT: f32 = 8.;
/// Minimum time between [`Changed`] events while a key is held.
const KEY_REPEAT_INTERVAL: Duration = Duration::from_millis(50);

/// The value and its limits. Pure, so the arithmetic is unit tested.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderModel {
    pub min: f64,
    pub max: f64,
    /// Arrow-key step; values snap to multiples of it from `min`. 0 disables
    /// snapping.
    pub step: f64,
    /// Shift+arrow and PageUp/PageDown.
    pub big_step: f64,
    /// Double-click restores this.
    pub default: f64,
    value: f64,
}

impl SliderModel {
    pub fn new(min: f64, max: f64, step: f64, default: f64) -> Self {
        let (min, max) = if min <= max { (min, max) } else { (max, min) };
        let mut model = Self {
            min,
            max,
            step: step.abs(),
            big_step: (step.abs() * 10.).min(max - min),
            default: default.clamp(min, max),
            value: min,
        };
        model.value = model.default;
        model
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    /// Sets the value, clamped and snapped. Returns whether it changed. NaN and
    /// infinite values are ignored.
    pub fn set(&mut self, value: f64) -> bool {
        if !value.is_finite() {
            return false;
        }
        let value = self.snap(value.clamp(self.min, self.max));
        let changed = value != self.value;
        self.value = value;
        changed
    }

    fn snap(&self, value: f64) -> f64 {
        if self.step <= 0. {
            return value;
        }
        let snapped = self.min + ((value - self.min) / self.step).round() * self.step;
        // Rounding can step past `max` when the range isn't a multiple of `step`.
        let snapped = snapped.clamp(self.min, self.max);
        // Keep 0.1 + 0.2 style noise out of displayed values.
        (snapped * 1e9).round() / 1e9
    }

    /// The value's position along the track, 0.0 to 1.0.
    pub fn fraction(&self) -> f64 {
        fraction_of(self.value, self.min, self.max)
    }

    pub fn set_fraction(&mut self, fraction: f64) -> bool {
        if !fraction.is_finite() {
            return false;
        }
        self.set(self.min + fraction.clamp(0., 1.) * (self.max - self.min))
    }

    pub fn step_by(&mut self, steps: f64, big: bool) -> bool {
        let step = if big { self.big_step } else { self.step };
        let step = if step > 0. {
            step
        } else {
            (self.max - self.min) / 100.
        };
        self.set(self.value + steps * step)
    }

    /// The filled part of the track as (start, end) fractions. Centered sliders
    /// fill from zero (or the range's nearest end, when zero is outside it).
    pub fn fill(&self, centered: bool) -> (f64, f64) {
        let value = self.fraction();
        if !centered {
            return (0., value);
        }
        let origin = fraction_of(0., self.min, self.max);
        (origin.min(value), origin.max(value))
    }
}

fn fraction_of(value: f64, min: f64, max: f64) -> f64 {
    if max > min {
        ((value - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    }
}

/// Lets at most one value through per `interval`, keeping the latest of the rest
/// for [`Self::flush`].
#[derive(Debug)]
pub struct Throttle {
    interval: Duration,
    last: Option<Instant>,
    pending: Option<f64>,
}

impl Throttle {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last: None,
            pending: None,
        }
    }

    /// Returns the value to send now, if any.
    pub fn offer(&mut self, value: f64, now: Instant) -> Option<f64> {
        if self
            .last
            .is_some_and(|last| now.duration_since(last) < self.interval)
        {
            self.pending = Some(value);
            return None;
        }
        self.last = Some(now);
        self.pending = None;
        Some(value)
    }

    /// The value held back since the last send, if any.
    pub fn flush(&mut self) -> Option<f64> {
        self.pending.take()
    }
}

/// Parses a number typed into a slider's field: ignores a trailing unit
/// ("200%", "-1.5 dB", "0.000 s") and accepts a comma as the decimal separator.
pub fn parse_number(text: &str) -> Option<f64> {
    let text = text.trim();
    let end = text
        .char_indices()
        .take_while(|(i, c)| {
            c.is_ascii_digit()
                || matches!(c, '.' | ',')
                || (*i == 0 && matches!(c, '-' | '+' | '−'))
        })
        .map(|(i, c)| i + c.len_utf8())
        .last()?;
    let number = text[..end].replace(',', ".").replace('−', "-");
    number.parse::<f64>().ok().filter(|v| v.is_finite())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// The value changed.
pub struct Changed(pub f64);

pub struct Slider {
    model: SliderModel,
    orientation: Orientation,
    centered: bool,
    accent: bool,
    label: SharedString,
    format: fn(f64) -> String,
    focus_handle: FocusHandle,
    track: BoundsTracker,
    dragging: bool,
    throttle: Throttle,
    field: Option<(gpui::Entity<TextInput>, Subscription)>,
}

impl EventEmitter<Changed> for Slider {}

impl Focusable for Slider {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Slider {
    pub fn new(
        label: impl Into<SharedString>,
        model: SliderModel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::observe(window, cx);
        Self {
            model,
            orientation: Orientation::Horizontal,
            centered: false,
            accent: false,
            label: label.into(),
            format: |v| format!("{v}"),
            focus_handle: cx.focus_handle().tab_stop(true),
            track: BoundsTracker::new(),
            dragging: false,
            throttle: Throttle::new(KEY_REPEAT_INTERVAL),
            field: None,
        }
    }

    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Fill from zero instead of from the minimum.
    pub fn centered(mut self) -> Self {
        self.centered = true;
        self
    }

    /// Accent fill and thumb (`Sepia Intensity`, the audio delay) instead of gray.
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }

    /// How the value reads in the drag tooltip and the numeric field.
    pub fn format(mut self, format: fn(f64) -> String) -> Self {
        self.format = format;
        self
    }

    /// Adds an editable numeric field next to the slider.
    pub fn with_field(mut self, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = (self.format)(self.model.value());
        let field = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(text)
                .with_label(self.label.clone())
        });
        let subscription = cx.subscribe_in(&field, window, |this, field, event, window, cx| {
            if let text_input::Event::Submitted | text_input::Event::Blurred = event {
                let text = field.read(cx).text().to_owned();
                if let Some(value) = parse_number(&text) {
                    this.set_from_user(value, cx);
                }
                // Show the value as accepted (clamped, snapped, formatted).
                this.sync_field(window, cx);
            }
        });
        self.field = Some((field, subscription));
        self
    }

    #[allow(dead_code)] // Kit API for later stages, exercised by the tests.
    pub fn value(&self) -> f64 {
        self.model.value()
    }

    /// Sets the value from outside (settings, the engine). Doesn't emit.
    #[allow(dead_code)] // Kit API: follows settings and the engine (Stage 1+).
    pub fn set_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dragging && self.model.set(value) {
            self.sync_field(window, cx);
            cx.notify();
        }
    }

    fn sync_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = (self.format)(self.model.value());
        if let Some((field, _)) = &self.field
            && !field.read(cx).is_focused(window)
        {
            field.update(cx, |field, cx| field.set_text(text, cx));
        }
    }

    fn set_from_user(&mut self, value: f64, cx: &mut Context<Self>) {
        if self.model.set(value) {
            cx.emit(Changed(self.model.value()));
            cx.notify();
        }
    }

    fn fraction_at(&self, position: Point<Pixels>) -> Option<f64> {
        let bounds = self.track.bounds()?;
        let fraction = match self.orientation {
            Orientation::Horizontal => (position.x - bounds.left()) / bounds.size.width.max(px(1.)),
            // Vertical sliders grow upward.
            Orientation::Vertical => {
                (bounds.bottom() - position.y) / bounds.size.height.max(px(1.))
            }
        };
        Some(fraction as f64)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        if event.click_count >= 2 {
            self.dragging = false;
            let default = self.model.default;
            self.set_from_user(default, cx);
            self.sync_field(window, cx);
            return;
        }
        self.dragging = true;
        if let Some(fraction) = self.fraction_at(event.position)
            && self.model.set_fraction(fraction)
        {
            cx.emit(Changed(self.model.value()));
            self.sync_field(window, cx);
        }
        cx.notify();
    }

    fn drag_to(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(fraction) = self.fraction_at(position)
            && self.model.set_fraction(fraction)
        {
            cx.emit(Changed(self.model.value()));
            self.sync_field(window, cx);
            cx.notify();
        }
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.dragging {
            self.dragging = false;
            cx.notify();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let m = keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return;
        }
        let big = m.shift;
        let (back, forward) = match self.orientation {
            Orientation::Horizontal => ("left", "right"),
            Orientation::Vertical => ("down", "up"),
        };
        let changed = match keystroke.key.as_str() {
            k if k == back => self.model.step_by(-1., big),
            k if k == forward => self.model.step_by(1., big),
            // The other axis's keys also work: Up/Right increase.
            "up" | "right" => self.model.step_by(1., big),
            "down" | "left" => self.model.step_by(-1., big),
            "pageup" => self.model.step_by(1., true),
            "pagedown" => self.model.step_by(-1., true),
            "home" => self.model.set(self.model.min),
            "end" => self.model.set(self.model.max),
            _ => return,
        };
        cx.stop_propagation();
        if !changed {
            return;
        }
        self.sync_field(window, cx);
        cx.notify();
        let now = Instant::now();
        if !event.is_held {
            // A fresh press always goes through.
            self.throttle = Throttle::new(KEY_REPEAT_INTERVAL);
        }
        if let Some(value) = self.throttle.offer(self.model.value(), now) {
            cx.emit(Changed(value));
        } else {
            // Send the held-back value once the repeat slows down or stops.
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(KEY_REPEAT_INTERVAL).await;
                this.update(cx, |this, cx| {
                    if let Some(value) = this.throttle.flush() {
                        cx.emit(Changed(value));
                    }
                })
                .ok();
            })
            .detach();
        }
    }
}

impl Render for Slider {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let horizontal = self.orientation == Orientation::Horizontal;
        let (fill_start, fill_end) = self.model.fill(self.centered);
        let fraction = self.model.fraction() as f32;
        let (fill_color, thumb_color) = if self.accent {
            (p.accent_tint, p.accent_strong)
        } else {
            (p.slider_fill, p.slider_thumb)
        };
        let value_text = SharedString::from((self.format)(self.model.value()));

        let fill = div()
            .absolute()
            .bg(fill_color)
            .rounded(px(TRACK / 2.))
            .map(|f| {
                let (start, len) = (fill_start as f32, (fill_end - fill_start) as f32);
                if horizontal {
                    f.top_0().bottom_0().left(relative(start)).w(relative(len))
                } else {
                    f.left_0()
                        .right_0()
                        .bottom(relative(start))
                        .h(relative(len))
                }
            });

        // A tick at zero on centered sliders (`video_colors`, the delay slider).
        let zero_tick = self.centered.then(|| {
            let zero = fraction_of(0., self.model.min, self.model.max) as f32;
            div().absolute().bg(p.divider).map(|t| {
                if horizontal {
                    t.top(px(3.)).bottom(px(3.)).w(px(2.)).left(relative(zero))
                } else {
                    t.left(px(3.))
                        .right(px(3.))
                        .h(px(2.))
                        .bottom(relative(zero))
                }
            })
        });

        let thumb = div()
            .absolute()
            .rounded(px(3.))
            .bg(thumb_color)
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
            .when(self.dragging, |t| {
                // The value tooltip while dragging.
                t.child(
                    div()
                        .absolute()
                        .bottom(px(THUMB_LONG + 6.))
                        .left(px(-30.))
                        .w(px(60. + THUMB_SHORT))
                        .flex()
                        .justify_center()
                        .child(
                            div()
                                .px(px(6.))
                                .py(px(2.))
                                .rounded(px(4.))
                                .bg(p.tooltip)
                                .text_size(px(12.))
                                .text_color(p.tooltip_text)
                                .whitespace_nowrap()
                                .child(value_text.clone()),
                        ),
                )
            });

        let track = div()
            .relative()
            .rounded(px(TRACK / 2.))
            .bg(p.well)
            .map(|t| {
                if horizontal {
                    t.h(px(TRACK)).w_full()
                } else {
                    t.w(px(TRACK)).h_full()
                }
            })
            .child(self.track.element())
            .child(fill)
            .children(zero_tick)
            .child(thumb);

        // While dragging, follow the mouse anywhere in the window.
        let drag_listener = self.dragging.then(|| {
            let entity = cx.entity();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let moved = entity.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase.bubble() && event.dragging() {
                            moved.update(cx, |this, cx| this.drag_to(event.position, window, cx));
                        }
                    });
                    let released = entity.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase.bubble() {
                            released.update(cx, |this, cx| this.end_drag(cx));
                        }
                    });
                },
            )
            .absolute()
            .size_0()
        });

        let slider = div()
            .id("slider")
            .flex()
            .items_center()
            .justify_center()
            .map(|s| {
                if horizontal {
                    s.flex_1()
                        .min_w(px(40.))
                        .h(px(THUMB_LONG + 6.))
                        .px(px(THUMB_SHORT / 2.))
                } else {
                    s.h_full()
                        .min_h(px(40.))
                        .w(px(THUMB_LONG + 6.))
                        .py(px(THUMB_SHORT / 2.))
                }
            })
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .track_focus(&self.focus_handle)
            .role(accesskit::Role::Slider)
            .aria_label(self.label.clone())
            .aria_numeric_value(self.model.value())
            .aria_min_numeric_value(self.model.min)
            .aria_max_numeric_value(self.model.max)
            .aria_numeric_value_step(self.model.step)
            .aria_value(value_text)
            .aria_orientation(if horizontal {
                accesskit::Orientation::Horizontal
            } else {
                accesskit::Orientation::Vertical
            })
            .cursor_pointer()
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .child(track)
            .children(drag_listener);
        let slider = focus::ring(slider, &p);

        match &self.field {
            Some((field, _)) => div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(12.))
                .w_full()
                .child(slider)
                .child(div().w(px(96.)).flex_none().child(field.clone()))
                .into_any_element(),
            None => slider.into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_and_snaps() {
        let mut m = SliderModel::new(0., 100., 5., 50.);
        assert_eq!(m.value(), 50.);
        assert!(m.set(42.));
        assert_eq!(m.value(), 40.);
        m.set(43.);
        assert_eq!(m.value(), 45.);
        m.set(1000.);
        assert_eq!(m.value(), 100.);
        m.set(-3.);
        assert_eq!(m.value(), 0.);
    }

    #[test]
    fn ignores_nan_and_infinity() {
        let mut m = SliderModel::new(0., 1., 0.1, 0.5);
        assert!(!m.set(f64::NAN));
        assert!(!m.set(f64::INFINITY));
        assert!(!m.set_fraction(f64::NAN));
        assert_eq!(m.value(), 0.5);
    }

    #[test]
    fn steps_without_float_noise() {
        let mut m = SliderModel::new(0., 1., 0.1, 0.);
        m.step_by(1., false);
        m.step_by(1., false);
        m.step_by(1., false);
        assert_eq!(m.value(), 0.3);
        // Big step is ten steps, clamped to the range.
        m.step_by(1., true);
        assert_eq!(m.value(), 1.);
    }

    #[test]
    fn snapping_never_passes_max() {
        // A range that isn't a multiple of the step.
        let mut m = SliderModel::new(0., 10., 3., 0.);
        m.set(9.9);
        assert_eq!(m.value(), 9.);
        m.set_fraction(1.);
        assert!(m.value() <= 10.);
    }

    #[test]
    fn reversed_limits_and_out_of_range_default() {
        let m = SliderModel::new(10., -10., 1., 50.);
        assert_eq!((m.min, m.max), (-10., 10.));
        assert_eq!(m.value(), 10.);
    }

    #[test]
    fn fraction_round_trip() {
        let mut m = SliderModel::new(-10., 10., 0., 0.);
        assert_eq!(m.fraction(), 0.5);
        m.set_fraction(0.75);
        assert_eq!(m.value(), 5.);
        // An empty range doesn't divide by zero.
        let empty = SliderModel::new(3., 3., 1., 3.);
        assert_eq!(empty.fraction(), 0.);
    }

    #[test]
    fn centered_fill_starts_at_zero() {
        let mut m = SliderModel::new(-12., 12., 1., 0.);
        m.set(6.);
        assert_eq!(m.fill(true), (0.5, 0.75));
        m.set(-6.);
        assert_eq!(m.fill(true), (0.25, 0.5));
        assert_eq!(m.fill(false), (0., 0.25));
        // Zero outside the range: fill from the nearest end.
        let mut positive = SliderModel::new(10., 20., 1., 15.);
        positive.set(15.);
        assert_eq!(positive.fill(true), (0., 0.5));
    }

    #[test]
    fn throttle_holds_back_bursts_and_flushes_the_latest() {
        let mut t = Throttle::new(Duration::from_millis(50));
        let t0 = Instant::now();
        assert_eq!(t.offer(1., t0), Some(1.));
        assert_eq!(t.offer(2., t0 + Duration::from_millis(10)), None);
        assert_eq!(t.offer(3., t0 + Duration::from_millis(20)), None);
        assert_eq!(t.flush(), Some(3.));
        assert_eq!(t.flush(), None);
        assert_eq!(t.offer(4., t0 + Duration::from_millis(60)), Some(4.));
    }

    #[test]
    fn parses_numbers_with_units() {
        assert_eq!(parse_number("200%"), Some(200.));
        assert_eq!(parse_number(" -1.5 dB"), Some(-1.5));
        assert_eq!(parse_number("0,25"), Some(0.25));
        assert_eq!(parse_number("−3"), Some(-3.));
        assert_eq!(parse_number("+2 s"), Some(2.));
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number(""), None);
        assert_eq!(parse_number("1e999"), Some(1.));
    }
}
