//! Mouse and keyboard handling for [`Slider`]: clicks, drags, double-click reset and
//! throttled arrow keys.

use std::time::Instant;

use gpui::{Context, KeyDownEvent, Keystroke, MouseDownEvent, Pixels, Point, Window, px};

use super::{Changed, Interaction, KEY_REPEAT_INTERVAL, Orientation, Slider, Throttle};

impl Slider {
    /// The track position under `position`, 0.0 to 1.0 (unclamped), once laid out.
    fn fraction_at(&self, position: Point<Pixels>) -> Option<f64> {
        let bounds = self.track.bounds()?;
        let fraction = match self.orientation {
            Orientation::Horizontal => (position.x - bounds.left()) / bounds.size.width.max(px(1.)),
            // Vertical sliders grow upward.
            Orientation::Vertical => {
                (bounds.bottom() - position.y) / bounds.size.height.max(px(1.))
            }
        };
        Some(f64::from(fraction))
    }

    /// Jumps to the clicked spot and starts a drag; a double-click resets instead.
    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window, cx);
        if event.click_count >= 2 {
            self.interaction = Interaction::Idle;
            let default = self.model.default;
            self.set_from_user(default, cx);
            self.sync_field(window, cx);
            return;
        }
        self.interaction = Interaction::Dragging;
        if let Some(fraction) = self.fraction_at(event.position)
            && self.model.set_fraction(fraction)
        {
            cx.emit(Changed(self.model.value()));
            self.sync_field(window, cx);
        }
        cx.notify();
    }

    /// Follows the mouse while dragging.
    pub(super) fn drag_to(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(fraction) = self.fraction_at(position)
            && self.model.set_fraction(fraction)
        {
            cx.emit(Changed(self.model.value()));
            self.sync_field(window, cx);
            cx.notify();
        }
    }

    /// Ends a drag when the mouse button is released anywhere.
    pub(super) fn end_drag(&mut self, cx: &mut Context<Self>) {
        if self.is_dragging() {
            self.interaction = Interaction::Idle;
            cx.notify();
        }
    }

    /// Steps the value with the arrow, page and Home/End keys.
    pub(super) fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(changed) = self.apply_key(&event.keystroke) else {
            return;
        };
        cx.stop_propagation();
        if !changed {
            return;
        }
        self.sync_field(window, cx);
        cx.notify();
        if !event.is_held {
            // A fresh press always goes through.
            self.throttle = Throttle::new(KEY_REPEAT_INTERVAL);
        }
        self.emit_throttled(cx);
    }

    /// Applies a key to the model. `None` when the slider doesn't handle the key,
    /// otherwise whether the value changed.
    fn apply_key(&mut self, keystroke: &Keystroke) -> Option<bool> {
        let m = keystroke.modifiers;
        if m.control || m.alt || m.platform {
            return None;
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
            _ => return None,
        };
        Some(changed)
    }

    /// Emits the value now if the throttle allows, or schedules the held-back value.
    fn emit_throttled(&mut self, cx: &mut Context<Self>) {
        if let Some(value) = self.throttle.offer(self.model.value(), Instant::now()) {
            cx.emit(Changed(value));
            return;
        }
        // Send the held-back value once the repeat slows down or stops.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(KEY_REPEAT_INTERVAL).await;
            this.update(cx, Self::flush_throttle).ok();
        })
        .detach();
    }

    /// Emits the value the throttle held back, if any.
    fn flush_throttle(&mut self, cx: &mut Context<Self>) {
        if let Some(value) = self.throttle.flush() {
            cx.emit(Changed(value));
        }
    }
}
