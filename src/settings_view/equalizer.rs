//! Settings → Audio's inline equalizer (`audio_02.png`, `eq-variant2.png`, S5.8.2, S7.7.2): an
//! "Open Equalizer" / "Close Equalizer" bar over eleven vertical sliders (the preamp, then the
//! ten bands), each named, with a box under it that shows its gain and takes a typed one
//! (S7.7.1).
//!
//! Moving a slider or typing a gain saves at once, and the player hears it at once. It also
//! turns the equalizer on and says Custom in the presets dropdown (S7.7.4), since the bands no
//! longer match a preset; the bands can be set while the Equalizer is off, as the design shows.
//! A box refuses text that isn't a gain (red, until it is fixed or left), and a gain past ±20 dB
//! is kept at the end. Whether the panel is open lasts for the session.

use gpui::{
    App, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*,
    px,
};

use crate::assets::icons;
use crate::engine::audio::equalizer::{FREQUENCIES, GAIN_RANGE, Preset, format_gain, parse_gain};
use crate::i18n::DirectionalStyle;
use crate::settings::{self, AudioSettings};
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::icon::icon;
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::text_input::{self, Look, TextInput, Validation};
use crate::ui::{focus, metrics};

/// The sliders' step, in dB.
const STEP: f64 = 0.1;
/// The sliders' height, and the room each column takes.
const SLIDER_HEIGHT: f32 = 300.;
const COLUMN_WIDTH: f32 = 60.;
/// The gain box's height and side padding.
const READOUT_HEIGHT: f32 = 30.;
const READOUT_PADDING: f32 = 6.;
/// The panel's inner padding and the space between its parts.
const PADDING: f32 = 20.;
const GAP: f32 = 12.;
/// The slider name before the bands.
const PREAMP: &str = tr_noop!("Preamp");
/// The longest text a gain box takes ("-12.5 dB" and some slack).
const FIELD_MAX_LEN: usize = 12;

/// One gain: its slider and the box under it.
struct Gain {
    slider: Entity<Slider>,
    field: Entity<TextInput>,
}

/// The equalizer panel.
pub(super) struct EqualizerPanel {
    /// The preamp's, then each band's.
    gains: Vec<Gain>,
    open: bool,
    _changes: Vec<Subscription>,
}

impl EqualizerPanel {
    /// The panel, showing the saved gains.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let saved = settings::get(cx).prefs.audio.equalizer_gains();
        let mut gains = Vec::new();
        let mut changes = Vec::new();
        for (index, gain) in saved.gains().enumerate() {
            let name = name(index);
            let slider = cx.new(|cx| band_slider(name, gain, window, cx));
            changes.push(cx.subscribe(&slider, move |_, _, event: &slider::Changed, cx| {
                set_gain(index, event.0, cx);
            }));
            let field = cx.new(|cx| {
                TextInput::new(window, cx)
                    .with_look(Look::Bare)
                    .with_text(format_gain(f64::from(gain)))
                    .with_label(tr!("{name} gain", name))
                    .with_max_len(FIELD_MAX_LEN)
            });
            changes.push(cx.subscribe(&field, move |_, field, event, cx| {
                on_field(index, &field, event, cx);
            }));
            gains.push(Gain { slider, field });
        }
        Self { gains, open: true, _changes: changes }
    }

    /// Shows the saved gains; a box being typed into keeps its text.
    pub(super) fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = settings::get(cx).prefs.audio.equalizer_gains();
        for (gain, value) in self.gains.iter().zip(saved.gains()) {
            let value = f64::from(value);
            gain.slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
            let field = gain.field.read(cx);
            if !field.is_focused(window) && !field.is_invalid() {
                show_gain(&gain.field, value, cx);
            }
        }
        cx.notify();
    }

    fn toggle_open(&mut self, cx: &mut Context<Self>) {
        self.open = !self.open;
        cx.notify();
    }

    /// The bar that opens and closes the panel.
    fn header(&self, p: &Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let (text, chevron) = if self.open {
            (tr!("Close Equalizer"), icons::CHEVRON_DOWN)
        } else {
            (
                tr!("Open Equalizer"),
                crate::i18n::mirrored(icons::CHEVRON_RIGHT, icons::CHEVRON_LEFT),
            )
        };
        let bar = div()
            .id("equalizer-toggle")
            .flex_row_start()
            .items_center()
            .justify_center()
            .gap(px(GAP))
            .h(px(READOUT_HEIGHT))
            .border_2()
            .border_color(gpui::transparent_black())
            .rounded(px(metrics::RADIUS))
            .cursor_pointer()
            .tab_index(0)
            .aria_expanded(self.open)
            .aria_label(text)
            .text_color(p.text)
            .child(text)
            .child(icon(chevron, 20., p.icon))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_open(cx)));
        focus::ring(bar, p)
    }

    /// A slider with its name and gain box under it.
    fn column(&self, index: usize, p: &Palette, cx: &App) -> impl IntoElement {
        let gain = &self.gains[index];
        let invalid = gain.field.read(cx).is_invalid();
        div()
            .w(px(COLUMN_WIDTH))
            .flex_row_start()
            .flex_col()
            .items_center()
            .gap(px(GAP))
            .child(div().h(px(SLIDER_HEIGHT)).child(gain.slider.clone()))
            .child(div().text_color(p.text_muted).child(name(index)))
            .child(
                div()
                    .h(px(READOUT_HEIGHT))
                    .w_full()
                    .flex_row_start()
                    .items_center()
                    .px(px(READOUT_PADDING))
                    .rounded(px(metrics::RADIUS))
                    .bg(p.well)
                    .border_1()
                    .border_color(if invalid { p.danger } else { p.well })
                    .text_size(text_px(metrics::TEXT_SMALL))
                    .text_color(p.text)
                    .child(gain.field.clone()),
            )
    }
}

impl Render for EqualizerPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let columns = self.open.then(|| {
            div()
                .flex_row_start()
                .justify_between()
                .children((0..self.gains.len()).map(|index| self.column(index, &p, cx)))
        });
        div()
            .w_full()
            .flex_row_start()
            .flex_col()
            .gap(px(GAP))
            .p(px(PADDING))
            .rounded(px(metrics::RADIUS))
            .bg(p.panel)
            .text_size(text_px(metrics::TEXT))
            .child(self.header(&p, cx))
            .children(columns)
    }
}

/// Gain `index`'s name: "Preamp", then the bands' frequencies.
fn name(index: usize) -> &'static str {
    index.checked_sub(1).map_or(crate::i18n::text(PREAMP), |band| {
        FREQUENCIES.get(band).copied().unwrap_or_default()
    })
}

/// A gain's slider, from −20 to 20 dB.
fn band_slider(name: &str, gain: f32, window: &mut Window, cx: &mut Context<Slider>) -> Slider {
    let mut model = SliderModel::new(f64::from(GAIN_RANGE.0), f64::from(GAIN_RANGE.1), STEP, 0.);
    model.set(f64::from(gain));
    Slider::new(SharedString::from(name.to_owned()), model, window, cx)
        .vertical()
        .accent()
        .format(format_gain)
}

/// Saves a typed gain on Enter or when the box is left; refuses text that isn't one, and puts
/// the saved gain back when the box is left holding it.
fn on_field(index: usize, field: &Entity<TextInput>, event: &text_input::Event, cx: &mut App) {
    let parsed = parse_gain(field.read(cx).text());
    match (event, parsed) {
        (text_input::Event::Changed, _) => {
            field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
        }
        (text_input::Event::Submitted | text_input::Event::Blurred, Ok(gain)) => {
            set_gain(index, gain, cx);
            show_gain(field, gain, cx);
        }
        (text_input::Event::Submitted, Err(why)) => field.update(cx, |field, cx| {
            field.set_validation(Validation::Error(why.into()), cx);
        }),
        (text_input::Event::Blurred, Err(_)) => {
            let saved = settings::get(cx).prefs.audio.equalizer_gains().gains().nth(index);
            show_gain(field, f64::from(saved.unwrap_or_default()), cx);
        }
    }
}

/// Shows `gain` in `field`, without an error.
fn show_gain(field: &Entity<TextInput>, gain: f64, cx: &mut App) {
    let text = format_gain(gain);
    field.update(cx, |field, cx| {
        if field.text() != text {
            field.set_text(text, cx);
        }
        field.set_validation(Validation::None, cx);
    });
}

/// Saves gain `index` (0 the preamp, then the bands).
fn set_gain(index: usize, gain: f64, cx: &mut App) {
    settings::update(cx, |settings| set(&mut settings.prefs.audio, index, gain as f32));
}

/// Sets gain `index`, which turns the equalizer on with the user's own gains (Custom).
fn set(audio: &mut AudioSettings, index: usize, gain: f32) {
    let gains = audio.equalizer_gains().with_gain(index, gain);
    audio.set_equalizer_gains(gains, Preset::Custom);
    audio.equalizer = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{EqualizerPick, Preferences};

    #[test]
    fn moving_a_band_keeps_the_others_and_says_custom() {
        let mut prefs = Preferences::default();
        let rock = Preset::Rock.equalizer().expect("a preset");
        prefs.pick_equalizer(&EqualizerPick::Preset(Preset::Rock));
        set(&mut prefs.audio, 3, 1.5);
        assert_eq!(prefs.equalizer_pick(), EqualizerPick::Preset(Preset::Custom));
        assert_eq!(prefs.audio.equalizer_bands[2], 1.5);
        assert_eq!(prefs.audio.equalizer_bands[0], rock.bands[0]);
        set(&mut prefs.audio, 0, -4.);
        assert_eq!(prefs.audio.equalizer_preamp, -4.);
    }

    #[test]
    fn moving_a_band_turns_the_equalizer_on() {
        let mut audio = AudioSettings::default();
        set(&mut audio, 1, 2.);
        assert!(audio.equalizer);
    }

    #[test]
    fn gains_are_named_preamp_then_by_frequency() {
        assert_eq!(name(0), "Preamp");
        assert_eq!(name(5), "1 kHz");
    }
}
