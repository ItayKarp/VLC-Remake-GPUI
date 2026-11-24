//! A value the settings panel edits two ways (S6.3.2, `controls-subtitle_settings.png`): a
//! labelled field that takes a typed value, and a slider under it. The subtitle delay, speed and
//! duration factor use it, and the audio delay will (S6.3.5, S7.6).
//!
//! The field may go past the slider's ends (a −30 s delay); the slider then rests at its end.
//! Text that isn't a value turns the field red with the reason and changes nothing; leaving the
//! field takes a good value and shows the value in use either way, and Escape puts it back. The
//! row only says what the user asked for ([`Changed`]): its owner decides and shows the value in
//! use with [`NumberRow::show`], so a refused or clamped change shows as what it became.

use gpui::{
    Context, Entity, EventEmitter, KeyDownEvent, Subscription, Window, div, prelude::*, px,
};

use crate::theme;
use crate::theme::text_px;
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::text_input::{self, TextInput, Validation};

/// The label row's height and text size, and the field's width.
const ROW_HEIGHT: f32 = 56.;
const ROW_TEXT: f32 = 14.;
const FIELD_WIDTH: f32 = 140.;
/// The space under the slider.
const SLIDER_BOTTOM: f32 = 8.;

/// A slider mark's label, from its value.
type MarkLabel = fn(f64) -> String;

/// What a row edits and how it reads and writes it.
#[derive(Clone, Copy)]
pub(in crate::main_window) struct Spec {
    /// The label, also the controls' name for screen readers.
    pub label: &'static str,
    /// The slider's range, steps and default (a double-click goes back to it).
    pub model: SliderModel,
    /// Filled from zero rather than from the minimum (a delay).
    pub centered: bool,
    /// Ticks every so far with labels under the track.
    pub marks: Option<(f64, MarkLabel)>,
    /// The value as the field shows it.
    pub format: fn(f64) -> String,
    /// A typed value, or why it isn't one.
    pub parse: fn(&str) -> Result<f64, &'static str>,
}

/// The user asked for this value.
pub(in crate::main_window) struct Changed(pub f64);

/// A labelled field and the slider under it.
pub(in crate::main_window) struct NumberRow {
    label: &'static str,
    format: fn(f64) -> String,
    parse: fn(&str) -> Result<f64, &'static str>,
    field: Entity<TextInput>,
    slider: Entity<Slider>,
    /// The value shown; `None` after a change, until the owner shows what it became.
    shown: Option<f64>,
    /// Enter took the typed value: the field shows it tidied ("-4,5" as "-4.500 s") once the
    /// owner shows it, though it keeps the focus.
    tidy: bool,
    disabled: bool,
    _events: [Subscription; 2],
}

impl EventEmitter<Changed> for NumberRow {}

impl NumberRow {
    /// A row showing `spec`'s default.
    pub(in crate::main_window) fn new(
        spec: Spec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::observe(window, cx);
        let value = spec.model.value();
        let slider = cx.new(|cx| {
            let mut slider =
                Slider::new(spec.label, spec.model, window, cx).accent().format(spec.format);
            if spec.centered {
                slider = slider.centered();
            }
            match spec.marks {
                Some((every, label)) => slider.marks(every, label),
                None => slider,
            }
        });
        let field = cx.new(|cx| {
            TextInput::new(window, cx).with_text((spec.format)(value)).with_label(spec.label)
        });
        let slid = cx.subscribe(&slider, |this, _, event: &slider::Changed, cx| {
            this.ask(event.0, cx);
        });
        let typed = cx.subscribe_in(&field, window, Self::on_field);
        Self {
            label: spec.label,
            format: spec.format,
            parse: spec.parse,
            field,
            slider,
            shown: Some(value),
            tidy: false,
            disabled: false,
            _events: [slid, typed],
        }
    }

    /// Shows `value`, the one in use. The field keeps what is being typed into it, unless Enter
    /// just took it.
    pub(in crate::main_window) fn show(
        &mut self,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.shown == Some(value) {
            return;
        }
        self.shown = Some(value);
        self.slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        if std::mem::take(&mut self.tidy) || !self.field.read(cx).is_focused(window) {
            self.show_text(cx);
        }
    }

    /// Fades the row and makes it take no input, or brings it back.
    pub(in crate::main_window) fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        if self.disabled == disabled {
            return;
        }
        self.disabled = disabled;
        self.slider.update(cx, |slider, cx| slider.set_disabled(disabled, cx));
        self.field.update(cx, |field, cx| field.set_disabled(disabled, cx));
    }

    /// Asks the owner for `value`.
    fn ask(&mut self, value: f64, cx: &mut Context<Self>) {
        self.shown = None;
        cx.emit(Changed(value));
    }

    fn on_field(
        &mut self,
        field: &Entity<TextInput>,
        event: &text_input::Event,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let parsed = (self.parse)(field.read(cx).text());
        match (event, parsed) {
            (text_input::Event::Changed, _) => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            (text_input::Event::Submitted, Ok(value)) => {
                self.tidy = true;
                self.ask(value, cx);
            }
            (text_input::Event::Submitted, Err(why)) => field.update(cx, |field, cx| {
                field.set_validation(Validation::Error(why.into()), cx);
            }),
            (text_input::Event::Blurred, parsed) => {
                if let Ok(value) = parsed {
                    self.ask(value, cx);
                }
                self.show_text(cx);
            }
        }
    }

    /// Escape puts back the value in use; once it shows, Escape goes on to close the panel.
    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
            return;
        }
        let Some(value) = self.shown else { return };
        let input = self.field.read(cx);
        if input.text() != (self.format)(value) || input.is_invalid() {
            self.show_text(cx);
            cx.stop_propagation();
        }
    }

    /// Shows the value in use in the field, without an error.
    fn show_text(&self, cx: &mut Context<Self>) {
        let Some(value) = self.shown else { return };
        let text = (self.format)(value);
        self.field.update(cx, |field, cx| {
            field.set_text(text, cx);
            field.set_validation(Validation::None, cx);
        });
    }
}

impl Render for NumberRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let field = div()
            .w(px(FIELD_WIDTH))
            .flex_none()
            .on_key_down(cx.listener(Self::on_key))
            .child(self.field.clone());
        let row = div()
            .h(px(ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .pl(px(4.))
            .text_size(text_px(ROW_TEXT))
            .text_color(p.text_muted)
            .child(self.label)
            .child(field);
        div().flex().flex_col().pb(px(SLIDER_BOTTOM)).child(row).child(self.slider.clone())
    }
}
