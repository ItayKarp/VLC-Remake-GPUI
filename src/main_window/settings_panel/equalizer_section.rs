//! The Audio tab's Equalizer section (S7.7, `panels/audio/equalizer/audio_equalizer-expanded`):
//! a title row with Reset and the toggle (D23) over a box that folds like the other effect
//! sections. Open, it holds the presets dropdown with each preset's curve and "Save as preset…",
//! the 2 Pass switch, then the preamp and ten bands stacked, each a label, a box that takes a
//! typed gain and a slider from −20 to +20 dB (S7.7.1, S7.7.2).
//!
//! Everything here changes the session (D24), as the Video tab's sections do; Save as Default
//! keeps it in Settings → Audio. The dropdown shows None while the equalizer is off, and picking
//! None turns it off ([`EqualizerPick`]). Moving a band says Custom (S7.7.4); a drag sends its
//! band on every move, so a preset picked meanwhile gives way to the drag until it ends.
//! "Save as preset…" names the gains in use and keeps them in Settings for every session.
//!
//! When the preamp and a band boost together past 20 dB a line under 2 Pass warns that loud
//! passages may distort; nothing is lowered behind the user's back.

use gpui::{
    App, AppContext as _, Context, Div, Entity, Focusable as _, SharedString, Subscription, Window,
    div, prelude::*, px,
};

use super::effect_section::{self, BOX_PADDING, TEXT};
use super::number_row::{Changed, NumberRow, Spec};
use crate::assets::equalizer_glyph;
use crate::effects::{self, AUDIO_EQUALIZER};
use crate::engine::audio::equalizer::{
    BANDS, Equalizer, FREQUENCIES, GAIN_RANGE, format_gain, parse_gain,
};
use crate::main_window::MainWindow;
use crate::settings::registry::Value;
use crate::settings::{self, EqualizerPick, MAX_SAVED_NAME};
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::slider::SliderModel;
use crate::ui::text_input::{self, TextInput, Validation};
use crate::ui::toggle::Toggle;

/// The section's settings.
const GAINS: &str = "audio.equalizer_bands";
const PRESET: &str = "audio.equalizer_preset";
const TWO_PASS: &str = "audio.equalizer_two_pass";
/// The label of the gain before the bands.
const PREAMP: &str = "Preamp";
/// The sliders' step, in dB.
const STEP: f64 = 0.1;
/// Past this much boost (the preamp plus the highest band), loud passages may distort.
const CLIP_RISK_DB: f32 = 20.;
/// The rows' height and the space between controls.
const ROW_HEIGHT: f32 = 48.;
const GAP: f32 = 8.;

/// The section's controls that keep state.
pub(super) struct EqualizerControls {
    presets: Entity<Dropdown>,
    /// The dropdown's entries, as listed.
    picks: Vec<EqualizerPick>,
    /// The preamp's row, then each band's.
    gains: Vec<Entity<NumberRow>>,
    /// The name field "Save as preset…" opened, and what it is listened to by.
    naming: Option<(Entity<TextInput>, Subscription)>,
    _events: Vec<Subscription>,
}

impl EqualizerControls {
    /// The controls, changing the session when used.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let picks = settings::get(cx).prefs.equalizer_picks();
        let labels = picks.iter().map(|pick| SharedString::from(pick.label())).collect();
        let glyphs = picks.iter().map(glyph).collect();
        let presets = cx.new(|cx| {
            let mut dropdown = Dropdown::new("Equalizer preset", labels, 0, window, cx);
            dropdown.set_glyphs(glyphs, cx);
            dropdown
        });
        let mut events = vec![cx.subscribe(&presets, |this, _, event: &dropdown::Changed, cx| {
            let pick = this.settings_panel.audio.equalizer.picks.get(event.0).cloned();
            if let Some(pick) = pick {
                pick_preset(&pick, cx);
            }
        })];
        let mut gains = Vec::new();
        for index in 0..=BANDS {
            let row = cx.new(|cx| NumberRow::new(gain_spec(index), window, cx));
            events.push(cx.subscribe(&row, move |_, _, event: &Changed, cx| {
                set_gain(index, event.0, cx);
            }));
            gains.push(row);
        }
        Self { presets, picks, gains, naming: None, _events: events }
    }

    /// Shows the values in effect, and the presets saved meanwhile.
    pub(super) fn sync(&mut self, window: &mut Window, cx: &mut Context<MainWindow>) {
        let effective = effects::effective(cx).prefs;
        let picks = effective.equalizer_picks();
        let selected = picks.iter().position(|pick| *pick == effective.equalizer_pick());
        if picks != self.picks {
            let labels = picks.iter().map(|pick| SharedString::from(pick.label())).collect();
            let glyphs = picks.iter().map(glyph).collect();
            self.presets.update(cx, |dropdown, cx| {
                dropdown.set_options(labels, cx);
                dropdown.set_glyphs(glyphs, cx);
            });
            self.picks = picks;
        }
        self.presets.update(cx, |dropdown, cx| dropdown.set_selected(selected.unwrap_or(0), cx));
        let gains = effective.audio.equalizer_gains();
        for (row, gain) in self.gains.iter().zip(gains.gains()) {
            row.update(cx, |row, cx| row.show(f64::from(gain), window, cx));
        }
    }

    /// The section: its title row, then its box.
    pub(super) fn render(&self, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
        let content = effect_section::is_expanded(cx, &AUDIO_EQUALIZER).then(|| {
            div()
                .flex()
                .flex_col()
                .px(px(BOX_PADDING))
                .pb(px(BOX_PADDING))
                .child(preset_row(p, cx))
                .child(self.presets.clone())
                .children(self.naming_row(p, cx))
                .child(two_pass_row(p, cx))
                .children(clip_warning(p, cx))
                .child(div().h(px(1.)).my(px(GAP)).bg(p.divider))
                .children(self.gains.iter().cloned())
        });
        effect_section::folding(&AUDIO_EQUALIZER, content, p, cx)
    }

    /// The name field "Save as preset…" opened, with Save and Cancel; `None` while closed.
    fn naming_row(&self, p: &Palette, cx: &mut Context<MainWindow>) -> Option<Div> {
        let (field, _) = self.naming.as_ref()?;
        let save = Button::new("save-equalizer-preset", "Save")
            .style(ButtonStyle::Subtle)
            .on_click(cx.listener(|this, _, _, cx| this.save_preset(cx)));
        let cancel = Button::new("cancel-equalizer-preset", "Cancel")
            .style(ButtonStyle::Ghost)
            .on_click(cx.listener(|this, _, _, cx| this.close_naming(cx)));
        let buttons = div().flex().justify_end().gap(px(GAP)).child(cancel).child(save);
        let fields = div().flex().flex_col().gap(px(GAP)).pt(px(GAP)).text_color(p.text);
        Some(fields.child(field.clone()).child(buttons))
    }
}

impl MainWindow {
    /// Opens the name field of "Save as preset…", focused.
    fn open_naming(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_placeholder("Preset name")
                .with_label("Preset name")
                .with_max_len(MAX_SAVED_NAME)
        });
        field.read(cx).focus_handle(cx).focus(window, cx);
        let typed = cx.subscribe(&field, |this, field, event, cx| match event {
            text_input::Event::Submitted => this.save_preset(cx),
            text_input::Event::Changed => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            text_input::Event::Blurred => {}
        });
        self.settings_panel.audio.equalizer.naming = Some((field, typed));
        cx.notify();
    }

    fn close_naming(&mut self, cx: &mut Context<Self>) {
        self.settings_panel.audio.equalizer.naming = None;
        cx.notify();
    }

    /// Saves the gains in effect under the typed name and picks the saved preset; a name that
    /// is empty is refused.
    fn save_preset(&mut self, cx: &mut Context<Self>) {
        let Some((field, _)) = &self.settings_panel.audio.equalizer.naming else { return };
        let field = field.clone();
        let name = field.read(cx).text().to_owned();
        let gains = effects::effective(cx).prefs.audio.equalizer_gains();
        let mut saved = None;
        settings::update(cx, |s| saved = s.prefs.save_equalizer(&name, gains));
        let Some(saved) = saved else {
            field.update(cx, |field, cx| {
                field.set_validation(Validation::Error("Give the preset a name".into()), cx);
            });
            return;
        };
        pick_preset(&EqualizerPick::Saved(saved), cx);
        self.close_naming(cx);
    }
}

/// "Preset", and "Save as preset…" at the right.
fn preset_row(p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let save = Button::new("save-as-equalizer-preset", "Save as preset…")
        .style(ButtonStyle::Ghost)
        .tooltip("Keeps the gains in use as a preset of your own", None::<&str>)
        .on_click(cx.listener(|this, _, window, cx| this.open_naming(window, cx)));
    labelled("Preset", p).child(save)
}

/// The 2 Pass switch's row.
fn two_pass_row(p: &Palette, cx: &App) -> Div {
    let on = effects::value(cx, TWO_PASS) == Some(Value::Bool(true));
    let switch = Toggle::new("equalizer-two-pass", on, "2 Pass").on_change(|on, _, cx| {
        effects::change(cx, &AUDIO_EQUALIZER, TWO_PASS, Value::Bool(*on));
    });
    labelled("2 Pass", p).child(switch)
}

/// A row with `label` at the left.
fn labelled(label: &'static str, p: &Palette) -> Div {
    div()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(GAP))
        .pl(px(4.))
        .text_size(px(TEXT))
        .text_color(p.text_muted)
        .child(label)
}

/// The line saying loud passages may distort, while the boosts in effect make that likely.
fn clip_warning(p: &Palette, cx: &App) -> Option<Div> {
    let gains = effects::effective(cx).prefs.audio.equalizer_gains();
    may_clip(&gains).then(|| {
        div()
            .pl(px(4.))
            .pb(px(GAP))
            .text_size(px(TEXT))
            .text_color(p.danger)
            .child("Loud passages may distort: lower the Preamp.")
    })
}

/// Whether the preamp and the highest band boost past [`CLIP_RISK_DB`] together.
fn may_clip(gains: &Equalizer) -> bool {
    let top = gains.bands.iter().copied().fold(0., f32::max);
    gains.preamp + top > CLIP_RISK_DB
}

/// Picks `pick` for the session: None turns the equalizer off, keeping the preset held for when
/// it comes back on; anything else turns it on.
fn pick_preset(pick: &EqualizerPick, cx: &mut App) {
    match pick {
        EqualizerPick::None => effects::set_enabled(cx, &AUDIO_EQUALIZER, false),
        pick => effects::change(cx, &AUDIO_EQUALIZER, PRESET, Value::Text(pick.id())),
    }
}

/// Sets gain `index` (0 the preamp, then the bands) for the session, which says Custom.
fn set_gain(index: usize, gain: f64, cx: &mut App) {
    let gains = effects::effective(cx).prefs.audio.equalizer_gains().with_gain(index, gain as f32);
    let numbers = gains.gains().map(f64::from).collect();
    effects::change(cx, &AUDIO_EQUALIZER, GAINS, Value::Numbers(numbers));
    let custom = EqualizerPick::Preset(crate::engine::audio::equalizer::Preset::Custom);
    effects::change(cx, &AUDIO_EQUALIZER, PRESET, Value::Text(custom.id()));
}

/// Gain `index`'s row: its label, a box taking a typed gain, and a slider from −20 to +20 dB.
fn gain_spec(index: usize) -> Spec {
    let label = index.checked_sub(1).map_or(PREAMP, |band| FREQUENCIES[band]);
    let (min, max) = (f64::from(GAIN_RANGE.0), f64::from(GAIN_RANGE.1));
    Spec {
        label,
        model: SliderModel::new(min, max, STEP, 0.),
        centered: false,
        marks: None,
        format: format_gain,
        parse: parse_gain,
    }
}

/// The curve before `pick` in the dropdown: VLC's presets have one.
fn glyph(pick: &EqualizerPick) -> Option<crate::assets::TwoTone> {
    match pick {
        EqualizerPick::Preset(preset) => equalizer_glyph(*preset),
        EqualizerPick::None | EqualizerPick::Saved(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;
    use crate::engine::audio::equalizer::Preset;

    fn pick(cx: &App) -> EqualizerPick {
        effects::effective(cx).prefs.equalizer_pick()
    }

    #[gpui::test]
    fn a_band_moved_after_a_preset_says_custom(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            pick_preset(&EqualizerPick::Preset(Preset::Rock), cx);
            assert_eq!(pick(cx), EqualizerPick::Preset(Preset::Rock));
            set_gain(2, 3., cx);
            assert_eq!(pick(cx), EqualizerPick::Preset(Preset::Custom));
            let gains = effects::effective(cx).prefs.audio.equalizer_gains();
            assert_eq!(gains.bands[1], 3.);
            assert_eq!(gains.bands[0], Preset::Rock.equalizer().expect("a preset").bands[0]);
            assert!(!settings::get(cx).prefs.audio.equalizer, "Settings keep their own");
        });
    }

    #[gpui::test]
    fn none_turns_the_equalizer_off_and_back_on_to_its_preset(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            pick_preset(&EqualizerPick::Preset(Preset::Pop), cx);
            pick_preset(&EqualizerPick::None, cx);
            assert_eq!(pick(cx), EqualizerPick::None);
            effects::set_enabled(cx, &AUDIO_EQUALIZER, true);
            assert_eq!(pick(cx), EqualizerPick::Preset(Preset::Pop));
        });
    }

    #[gpui::test]
    fn save_as_default_keeps_the_band_over_the_preset(cx: &mut TestAppContext) {
        cx.update(|cx| {
            settings::init_in_memory(cx);
            pick_preset(&EqualizerPick::Preset(Preset::Rock), cx);
            set_gain(0, -2., cx);
            effects::save_as_default(cx, AUDIO_EQUALIZER.keys());
            let audio = settings::get(cx).prefs.audio.clone();
            assert!(audio.equalizer);
            assert_eq!(audio.equalizer_preset, Preset::Custom);
            assert_eq!(audio.equalizer_preamp, -2.);
        });
    }

    #[test]
    fn only_strong_boosts_warn() {
        assert!(!may_clip(&Equalizer::FLAT));
        for preset in Preset::ALL {
            assert!(!may_clip(&preset.equalizer().expect("a preset")), "{preset:?}");
        }
        assert!(may_clip(&Equalizer::FLAT.with_gain(1, 10.)));
    }
}
