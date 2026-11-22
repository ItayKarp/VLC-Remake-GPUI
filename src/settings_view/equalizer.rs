//! Settings → Audio's inline equalizer (`audio_02.png`, S5.8.2): a "Close Equalizer" bar over
//! eleven vertical sliders (the preamp, then the ten bands), each named and with its gain under
//! it.
//!
//! Moving a slider saves at once, and the player hears it at once; it also clears the preset,
//! since the bands no longer match it. The bands can be set while the Equalizer is off, as the
//! design shows, and are heard once it is on. Whether the panel is open lasts for the session.

use gpui::{
    App, Context, Entity, IntoElement, Render, SharedString, Subscription, Window, div, prelude::*,
    px,
};

use crate::assets::icons;
use crate::engine::audio::equalizer::{BANDS, FREQUENCIES, GAIN_RANGE, Preset};
use crate::settings::{self, AudioSettings};
use crate::theme::{self, Palette};
use crate::ui::icon::icon;
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::{focus, metrics};

/// The sliders' step, in dB.
const STEP: f64 = 0.1;
/// The sliders' height, and the room each column takes.
const SLIDER_HEIGHT: f32 = 300.;
const COLUMN_WIDTH: f32 = 56.;
/// The gain readout's box.
const READOUT_HEIGHT: f32 = 30.;
/// The panel's inner padding and the space between its parts.
const PADDING: f32 = 20.;
const GAP: f32 = 12.;
/// The slider name before the bands.
const PREAMP: &str = "Preamp";

/// The equalizer panel.
pub(super) struct EqualizerPanel {
    /// The preamp's slider, then each band's.
    sliders: Vec<Entity<Slider>>,
    open: bool,
    _changes: Vec<Subscription>,
}

impl EqualizerPanel {
    /// The panel, showing the saved gains.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let audio = settings::get(cx).prefs.audio.clone();
        let gains = std::iter::once(audio.equalizer_preamp).chain(audio.equalizer_bands);
        let names = std::iter::once(PREAMP).chain(FREQUENCIES);
        let mut sliders = Vec::new();
        let mut changes = Vec::new();
        for (index, (name, gain)) in names.zip(gains).enumerate() {
            let slider = cx.new(|cx| band_slider(name, gain, window, cx));
            changes.push(cx.subscribe(&slider, move |_, _, event: &slider::Changed, cx| {
                set_gain(index, event.0 as f32, cx);
            }));
            sliders.push(slider);
        }
        Self { sliders, open: true, _changes: changes }
    }

    /// Shows the saved gains.
    pub(super) fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let audio = settings::get(cx).prefs.audio.clone();
        let gains = std::iter::once(audio.equalizer_preamp).chain(audio.equalizer_bands);
        for (slider, gain) in self.sliders.iter().zip(gains) {
            slider.update(cx, |slider, cx| slider.set_value(f64::from(gain), window, cx));
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
            ("Close Equalizer", icons::CHEVRON_DOWN)
        } else {
            ("Open Equalizer", icons::CHEVRON_RIGHT)
        };
        let bar = div()
            .id("equalizer-toggle")
            .flex()
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

    /// A slider with its name and gain under it.
    fn column(&self, index: usize, p: &Palette, cx: &App) -> impl IntoElement {
        let slider = &self.sliders[index];
        let gain = slider.read(cx).value();
        let name = if index == 0 { PREAMP } else { FREQUENCIES[index - 1] };
        div()
            .w(px(COLUMN_WIDTH))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(GAP))
            .child(div().h(px(SLIDER_HEIGHT)).child(slider.clone()))
            .child(div().text_color(p.text_muted).child(name))
            .child(
                div()
                    .h(px(READOUT_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(metrics::RADIUS))
                    .bg(p.well)
                    .text_size(px(metrics::TEXT_SMALL))
                    .text_color(p.text)
                    .child(SharedString::from(decibels(gain))),
            )
    }
}

impl Render for EqualizerPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let columns = self.open.then(|| {
            div()
                .flex()
                .justify_between()
                .children((0..self.sliders.len()).map(|index| self.column(index, &p, cx)))
        });
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(GAP))
            .p(px(PADDING))
            .rounded(px(metrics::RADIUS))
            .bg(p.panel)
            .text_size(px(metrics::TEXT))
            .child(self.header(&p, cx))
            .children(columns)
    }
}

/// A gain's slider, from −20 to 20 dB.
fn band_slider(name: &str, gain: f32, window: &mut Window, cx: &mut Context<Slider>) -> Slider {
    let mut model = SliderModel::new(f64::from(GAIN_RANGE.0), f64::from(GAIN_RANGE.1), STEP, 0.);
    model.set(f64::from(gain));
    Slider::new(SharedString::from(name.to_owned()), model, window, cx)
        .vertical()
        .accent()
        .format(decibels)
}

/// Saves slider `index`'s gain (0 the preamp, then the bands), leaving the preset.
fn set_gain(index: usize, gain: f32, cx: &mut App) {
    settings::update(cx, |settings| set(&mut settings.prefs.audio, index, gain));
}

fn set(audio: &mut AudioSettings, index: usize, gain: f32) {
    let mut gains = audio.equalizer_gains();
    match index.checked_sub(1) {
        None => gains.preamp = gain,
        Some(band) if band < BANDS => gains.bands[band] = gain,
        Some(_) => return,
    }
    audio.set_equalizer_gains(gains, Preset::None);
}

/// A gain as the panel shows it: "12.0 dB".
fn decibels(gain: f64) -> String {
    format!("{gain:.1} dB")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_a_band_keeps_the_others_and_clears_the_preset() {
        let mut audio = AudioSettings::default();
        let rock = Preset::Rock.equalizer().expect("a preset");
        audio.set_equalizer_gains(rock, Preset::Rock);
        set(&mut audio, 3, 1.5);
        assert_eq!(audio.equalizer_preset, Preset::None);
        assert_eq!(audio.equalizer_bands[2], 1.5);
        assert_eq!(audio.equalizer_bands[0], rock.bands[0]);
        set(&mut audio, 0, -4.);
        assert_eq!(audio.equalizer_preamp, -4.);
    }

    #[test]
    fn gains_read_in_tenths_of_a_decibel() {
        assert_eq!(decibels(12.), "12.0 dB");
        assert_eq!(decibels(-3.), "-3.0 dB");
    }
}
