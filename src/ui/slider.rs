//! Sliders (S0.4.5): `player/panels/video/colors/video_colors-expanded`
//! (Saturation, Sepia Intensity), the audio delay slider and the EQ bands.
//!
//! Horizontal or vertical; a neutral or accent fill; filled from the minimum or
//! from zero ("centered", for balance, delay and EQ gain). Drag the thumb or click
//! the track to jump; arrow keys step (Shift for a big step), Home/End/PageUp/
//! PageDown work too; double-click resets to the default. While dragging, a
//! tooltip shows the value. An optional numeric field edits it directly, and
//! optional marks put ticks on the track and labels under it (the speed slider,
//! `controls-playback.png`). A disabled slider is faded and ignores input.
//!
//! Values from settings or the engine are clamped for display, and NaN/infinite
//! values are ignored. Held keys auto-repeat, but [`Changed`] is throttled so the
//! engine isn't flooded; the final value always arrives.

mod input;
mod marks;
mod model;
mod parse;
mod render;
mod throttle;

use std::time::Duration;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, SharedString, Subscription, Window,
    prelude::*,
};

pub use model::SliderModel;
pub use parse::parse_number;
pub use throttle::Throttle;

use super::popover::BoundsTracker;
use super::text_input::{self, TextInput};
use crate::theme;

/// Minimum time between [`Changed`] events while a key is held.
const KEY_REPEAT_INTERVAL: Duration = Duration::from_millis(50);

/// Which way the track runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Left to right.
    Horizontal,
    /// Bottom to top.
    Vertical,
}

/// The colors of the fill and thumb.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    /// Gray, for most sliders.
    Neutral,
    /// The accent color (`Sepia Intensity`, the audio delay).
    Accent,
}

/// The value changed.
pub struct Changed(pub f64);

/// What the pointer and keys can do with the slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interaction {
    /// Ready for input.
    Idle,
    /// The thumb follows the mouse.
    Dragging,
    /// Faded; takes no input.
    Disabled,
}

/// Ticks along the track every `every` from the minimum, each labelled under the track.
#[derive(Clone, Copy)]
struct Marks {
    every: f64,
    label: fn(f64) -> String,
}

/// A slider with an optional numeric field; emits [`Changed`] when the user moves it.
pub struct Slider {
    model: SliderModel,
    orientation: Orientation,
    centered: bool,
    tone: Tone,
    label: SharedString,
    format: fn(f64) -> String,
    focus_handle: FocusHandle,
    track: BoundsTracker,
    interaction: Interaction,
    throttle: Throttle,
    field: Option<(Entity<TextInput>, Subscription)>,
    marks: Option<Marks>,
}

impl EventEmitter<Changed> for Slider {}

impl Focusable for Slider {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Slider {
    /// Creates a horizontal, neutral slider named `label` for screen readers.
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
            tone: Tone::Neutral,
            label: label.into(),
            format: |v| format!("{v}"),
            focus_handle: cx.focus_handle().tab_stop(true),
            track: BoundsTracker::new(),
            interaction: Interaction::Idle,
            throttle: Throttle::new(KEY_REPEAT_INTERVAL),
            field: None,
            marks: None,
        }
    }

    /// Runs the track bottom to top (the EQ bands).
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
        self.tone = Tone::Accent;
        self
    }

    /// How the value reads in the drag tooltip and the numeric field.
    pub fn format(mut self, format: fn(f64) -> String) -> Self {
        self.format = format;
        self
    }

    /// Puts a tick on the track every `every` from the minimum, and each mark's `label` under
    /// the track. Horizontal sliders only.
    pub fn marks(mut self, every: f64, label: fn(f64) -> String) -> Self {
        self.marks = Some(Marks { every, label });
        self
    }

    /// Adds an editable numeric field next to the slider.
    pub fn with_field(mut self, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let text = (self.format)(self.model.value());
        let field =
            cx.new(|cx| TextInput::new(window, cx).with_text(text).with_label(self.label.clone()));
        let subscription = cx.subscribe_in(&field, window, |this, field, event, window, cx| {
            this.on_field_event(field, event, window, cx);
        });
        self.field = Some((field, subscription));
        self
    }

    /// The current value.
    #[allow(dead_code, reason = "kit API for later stages, exercised by the tests")]
    pub fn value(&self) -> f64 {
        self.model.value()
    }

    /// Sets the value from outside (settings, the engine). Doesn't emit.
    #[allow(dead_code, reason = "kit API: follows settings and the engine (Stage 1+)")]
    pub fn set_value(&mut self, value: f64, window: &mut Window, cx: &mut Context<Self>) {
        if self.interaction != Interaction::Dragging && self.model.set(value) {
            self.sync_field(window, cx);
            cx.notify();
        }
    }

    /// Enables or disables the slider (and its field); a disabled slider is faded and takes no
    /// focus, keys or mouse.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.is_disabled() == disabled {
            return;
        }
        self.interaction = if disabled { Interaction::Disabled } else { Interaction::Idle };
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        if let Some((field, _)) = &self.field {
            field.update(cx, |field, cx| field.set_disabled(disabled, cx));
        }
        cx.notify();
    }

    fn is_disabled(&self) -> bool {
        self.interaction == Interaction::Disabled
    }

    fn is_dragging(&self) -> bool {
        self.interaction == Interaction::Dragging
    }

    /// Takes the field's number when the user submits it or leaves the field.
    fn on_field_event(
        &mut self,
        field: &Entity<TextInput>,
        event: &text_input::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, text_input::Event::Submitted | text_input::Event::Blurred) {
            return;
        }
        let text = field.read(cx).text().to_owned();
        if let Some(value) = parse_number(&text) {
            self.set_from_user(value, cx);
        }
        // Show the value as accepted (clamped, snapped, formatted).
        self.sync_field(window, cx);
    }

    /// Shows the current value in the field, unless the user is typing in it.
    fn sync_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = (self.format)(self.model.value());
        if let Some((field, _)) = &self.field
            && !field.read(cx).is_focused(window)
        {
            field.update(cx, |field, cx| field.set_text(text, cx));
        }
    }

    /// Sets the value on the user's behalf, emitting [`Changed`] if it moved.
    fn set_from_user(&mut self, value: f64, cx: &mut Context<Self>) {
        if self.model.set(value) {
            cx.emit(Changed(self.model.value()));
            cx.notify();
        }
    }
}
