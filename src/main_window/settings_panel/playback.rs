//! The settings panel's Playback tab (S2.7.2, `controls-playback.png`): Show Subtitle Timeline on
//! Seekbar (S6.6), Show Statistics (S8.13.2), the Sleep Timer (`sleep_timer_row.rs`, S8.13.1),
//! the Audio Visualizer (`visualizer_row.rs`, S7.8.1), and the speed, as a field ("1x") and a
//! slider from 0.25× to 2× with a tick and a label every quarter.
//!
//! The timeline toggle is kept across launches (`prefs.interface.subtitle_timeline`). With no
//! subtitle showing, or one whose times aren't known, it is disabled with a line saying why.
//!
//! The field takes up to 4× ([`rules::parse`] reads `1.5`, `1,5`, `1.5x`, `150%`); the slider
//! then rests at its 2× end while the field shows the speed. Text that isn't a speed turns the
//! field red and changes nothing; Escape puts the speed back before it closes the panel. Live
//! streams, which can't change speed, show both disabled.

use gpui::{Context, Div, Entity, KeyDownEvent, Subscription, Window, div, prelude::*, px};

use crate::engine::values::MIN_RATE;
use crate::main_window::MainWindow;
use crate::main_window::speed::rules::{self, STEP};
use crate::settings;
use crate::theme::Palette;
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::text_input::{self, TextInput, Validation};
use crate::ui::toggle::Toggle;

/// The slider's fastest speed; the field goes further.
const SLIDER_MAX: f64 = 2.;
/// How finely the slider and its arrow keys move.
const SLIDER_STEP: f64 = 0.05;
/// The Speed row's height and text size, and the field's width.
const ROW_HEIGHT: f32 = 64.;
const ROW_TEXT: f32 = 14.;
const FIELD_WIDTH: f32 = 140.;
/// The tab's padding.
const PADDING: f32 = 20.;
/// The toggle row's height, and its hint's text size.
const TOGGLE_ROW_HEIGHT: f32 = 52.;
const HINT_TEXT: f32 = 12.;
/// The toggles' labels.
const TIMELINE_LABEL: &str = "Show Subtitle Timeline on Seekbar";
const STATISTICS_LABEL: &str = "Show Statistics";

/// The speed field and slider, and what they last showed.
pub(in crate::main_window) struct SpeedControls {
    slider: Entity<Slider>,
    field: Entity<TextInput>,
    /// The speed the controls show, so they're only updated when it changes.
    shown: Option<f32>,
    disabled: bool,
    _events: [Subscription; 2],
}

impl SpeedControls {
    /// The controls at normal speed.
    pub(in crate::main_window) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let slider = cx.new(|cx| {
            let mut model = SliderModel::new(f64::from(MIN_RATE), SLIDER_MAX, SLIDER_STEP, 1.);
            model.big_step = f64::from(STEP);
            Slider::new("Speed", model, window, cx)
                .accent()
                .format(rules::field_text)
                .marks(f64::from(STEP), rules::number)
        });
        let field = cx.new(|cx| {
            TextInput::new(window, cx).with_text(rules::field_text(1.)).with_label("Speed")
        });
        let slid = cx.subscribe_in(&slider, window, |this, _, event: &slider::Changed, _, cx| {
            this.set_speed(event.0 as f32, cx);
        });
        let typed = cx.subscribe_in(&field, window, MainWindow::on_speed_field);
        Self { slider, field, shown: None, disabled: false, _events: [slid, typed] }
    }
}

impl MainWindow {
    /// The Playback tab: the timeline toggle, then the Speed row and the slider under it.
    pub(super) fn render_playback_tab(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        self.sync_speed_controls(window, cx);
        let speed = &self.settings_panel.speed;
        let field = div()
            .w(px(FIELD_WIDTH))
            .flex_none()
            .on_key_down(cx.listener(Self::on_speed_field_key))
            .child(speed.field.clone());
        let row = div()
            .h(px(ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .pl(px(4.))
            .text_size(px(ROW_TEXT))
            .text_color(p.text_muted)
            .child("Speed")
            .child(field);
        div()
            .flex()
            .flex_col()
            .px(px(PADDING))
            .child(self.render_timeline_toggle(p, cx))
            .child(self.render_statistics_toggle(p, cx))
            .child(self.render_sleep_timer(p, cx))
            .child(self.settings_panel.visualizer.render(p, cx))
            .child(row)
            .child(speed.slider.clone())
    }

    /// Show Subtitle Timeline on Seekbar, and why it is disabled when it is.
    fn render_timeline_toggle(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let on = settings::get(cx).prefs.interface.subtitle_timeline;
        let unavailable = self.subtitle_timeline_unavailable(cx);
        let toggle = Toggle::new("subtitle-timeline", on, TIMELINE_LABEL)
            .disabled(unavailable.is_some())
            .on_change(|on, _, cx| {
                settings::update(cx, |s| s.prefs.interface.subtitle_timeline = *on);
            });
        let row = toggle_row(TIMELINE_LABEL, toggle, p);
        let hint = unavailable.map(|why| {
            div()
                .pl(px(4.))
                .pb(px(8.))
                .text_size(px(HINT_TEXT))
                .text_color(p.text_muted)
                .child(why.hint())
        });
        div().flex().flex_col().child(row).children(hint)
    }

    /// Show Statistics, which shows the overlay over the video (`statistics.rs`).
    fn render_statistics_toggle(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let toggle = Toggle::new("show-statistics", self.shows_statistics(), STATISTICS_LABEL)
            .on_change(cx.listener(|this, on: &bool, _, cx| this.set_show_statistics(*on, cx)));
        toggle_row(STATISTICS_LABEL, toggle, p)
    }

    /// Shows the player's speed in the controls, and disables them for live streams.
    fn sync_speed_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let player = self.loaded_player(cx).map(|player| player.read(cx).rate().get());
        let disabled = self.speed_player(cx).is_none();
        let speed = &mut self.settings_panel.speed;
        if speed.disabled != disabled {
            speed.disabled = disabled;
            speed.slider.update(cx, |slider, cx| slider.set_disabled(disabled, cx));
            speed.field.update(cx, |field, cx| field.set_disabled(disabled, cx));
        }
        let Some(rate) = player else { return };
        if speed.shown == Some(rate) {
            return;
        }
        speed.shown = Some(rate);
        let value = f64::from(rate);
        speed.slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
        if !speed.field.read(cx).is_focused(window) {
            Self::show_speed_text(&speed.field, rate, cx);
        }
    }

    fn on_speed_field(
        &mut self,
        field: &Entity<TextInput>,
        event: &text_input::Event,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            text_input::Event::Changed => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            text_input::Event::Submitted => match rules::parse(field.read(cx).text()) {
                Ok(speed) => {
                    self.set_speed(speed, cx);
                    Self::show_speed_text(field, speed, cx);
                }
                Err(rules::Invalid) => field.update(cx, |field, cx| {
                    field.set_validation(Validation::Error("Not a speed".into()), cx);
                }),
            },
            // Leaving the field takes a valid speed, and shows the speed playing either way.
            text_input::Event::Blurred => {
                if let Ok(speed) = rules::parse(field.read(cx).text()) {
                    self.set_speed(speed, cx);
                }
                self.revert_speed_field(cx);
            }
        }
    }

    /// Escape puts back the speed playing; once it shows, Escape closes the panel.
    fn on_speed_field_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
            return;
        }
        if self.revert_speed_field(cx) {
            cx.stop_propagation();
        }
    }

    /// Shows the speed playing in the field, without an error. Returns whether that changed it.
    fn revert_speed_field(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(player) = self.loaded_player(cx) else { return false };
        let speed = player.read(cx).rate().get();
        let field = self.settings_panel.speed.field.clone();
        let input = field.read(cx);
        let changed = input.text() != rules::field_text(f64::from(speed)) || input.is_invalid();
        if changed {
            Self::show_speed_text(&field, speed, cx);
        }
        changed
    }

    fn show_speed_text(field: &Entity<TextInput>, speed: f32, cx: &mut Context<Self>) {
        field.update(cx, |field, cx| {
            field.set_text(rules::field_text(f64::from(speed)), cx);
            field.set_validation(Validation::None, cx);
        });
    }
}

/// A toggle's row: its label, then the toggle.
fn toggle_row(label: &'static str, toggle: Toggle, p: &Palette) -> Div {
    div()
        .h(px(TOGGLE_ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .pl(px(4.))
        .text_size(px(ROW_TEXT))
        .text_color(p.text_muted)
        .child(label)
        .child(toggle)
}
