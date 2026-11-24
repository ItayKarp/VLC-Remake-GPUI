//! The settings panel's Audio tab (S7.6, `controls-audio_settings.png`): **Defaults** for media
//! opened next (Audio Track and Audio Language, `track_defaults.rs`) and the **Stereo Mode** with
//! its **Test**, then **Synchronization**: Audio Track Delay, a field to the millisecond over a
//! −10 s … +10 s slider filled from 0 s (the design's right label reads "-10 s", a typo).
//!
//! The stereo mode changes what plays for the session (D24), over Settings → Audio's Stereo mode;
//! Save as Default keeps it there. Test plays a tone on the left, then the right, through the mode
//! in use at a fixed level, pausing the media meanwhile (`engine/player/test_tone.rs`), and says
//! which side sounds over the video. The delay belongs to the audio track playing (S7.6.3), as
//! the J and K keys' does; with no audio playing the row is disabled. The effect sections follow:
//! the Equalizer (`equalizer_section.rs`, S7.7), then the Compressor, Spatializer and Stereo
//! Widener (S7.9), sliders like the Video tab's sections (`effect_section.rs`), in the order sound
//! goes through them. A line over them says when passthrough is on, as sound sent undecoded
//! skips them, and one under the Stereo Widener when the track playing is mono, which it can't
//! widen.

use gpui::{
    App, AppContext as _, Context, Div, Entity, SharedString, Subscription, Window, div,
    prelude::*, px,
};

use super::effect_section::EffectControls;
use super::equalizer_section::EqualizerControls;
use super::number_row::{Changed, NumberRow};
use super::subtitles::{MS_PER_SECOND, track_delay_spec};
use super::track_defaults::{DefaultsRows, TrackDefaults};
use super::{PanelTab, SECTION_NOTE_GAP, SECTION_NOTE_TEXT, SECTION_PADDING, section};
use crate::effects;
use crate::engine::audio::test_tone;
use crate::engine::player::Player;
use crate::engine::values::Offset;
use crate::main_window::MainWindow;
use crate::settings::registry::{self, Choice, Control, Value};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::dropdown::{self, Dropdown};

/// The setting behind Stereo Mode.
const STEREO_MODE: &str = "audio.stereo_mode";
/// The Stereo Mode row's height and text size, and the space under its dropdown.
const ROW_HEIGHT: f32 = 60.;
const ROW_TEXT: f32 = 14.;
const NOTE_GAP: f32 = 8.;
/// The space around the effect sections.
const EFFECTS_PADDING: f32 = 8.;
/// What the effects can't do with passthrough on, and with a mono track.
const PASSTHROUGH_NOTE: &str = "Passthrough is on: AC3 and DTS sound reaches the receiver \
                                undecoded, without these effects.";
const MONO_NOTE: &str = "This track is mono, so the Stereo Widener has nothing to widen.";

/// The tab's controls.
pub(in crate::main_window) struct AudioControls {
    pub(super) defaults: DefaultsRows,
    stereo: Entity<Dropdown>,
    delay: Entity<NumberRow>,
    pub(super) equalizer: EqualizerControls,
    _events: [Subscription; 2],
}

impl AudioControls {
    /// The controls, showing the values in effect.
    pub(in crate::main_window) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let labels = stereo_choices().iter().map(|choice| SharedString::from(choice.label));
        let selected = stereo_index(cx);
        let stereo =
            cx.new(|cx| Dropdown::new("Stereo Mode", labels.collect(), selected, window, cx));
        let picked = cx.subscribe(&stereo, |_, _, event: &dropdown::Changed, cx| {
            if let Some(choice) = stereo_choices().get(event.0) {
                effects::set(cx, STEREO_MODE, Value::Choice(choice.id));
            }
        });
        let delay = cx.new(|cx| NumberRow::new(track_delay_spec("Audio Track Delay"), window, cx));
        let moved = cx.subscribe(&delay, |this, _, event: &Changed, cx| {
            let millis = (event.0 * MS_PER_SECOND).round() as i64;
            this.set_audio_delay(Offset::from_millis(millis), cx);
            cx.notify();
        });
        let defaults = DefaultsRows::new(TrackDefaults::Audio, window, cx);
        let equalizer = EqualizerControls::new(window, cx);
        Self { defaults, stereo, delay, equalizer, _events: [picked, moved] }
    }
}

impl MainWindow {
    /// The Audio tab: Defaults, Synchronization, then the Equalizer.
    pub(super) fn render_audio_tab(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        self.sync_audio_controls(window, cx);
        let controls = &self.settings_panel.audio;
        let defaults = section(
            "Defaults",
            "For videos you open next. The audio button picks this video's.",
            p,
        )
        .children(controls.defaults.render(p, cx))
        .child(stereo_row(p, cx))
        .child(controls.stereo.clone())
        .child(stereo_note(p));
        let target = self.audio_target(cx);
        let note =
            target.unwrap_or_else(|| "Pick an audio track to sync it with the video.".into());
        let sync = section("Synchronization", note, p).child(controls.delay.clone());
        let effects = div()
            .flex()
            .flex_col()
            .py(px(EFFECTS_PADDING))
            .px(px(SECTION_PADDING))
            .children(
                effects::effective(cx)
                    .prefs
                    .audio
                    .passthrough
                    .then(|| effects_note(PASSTHROUGH_NOTE, p)),
            )
            .child(controls.equalizer.render(p, cx))
            .children(self.audio_sections().map(|controls| controls.render(p, cx)))
            .children(self.plays_mono(cx).then(|| effects_note(MONO_NOTE, p)));
        div()
            .flex()
            .flex_col()
            .child(defaults)
            .child(div().h(px(1.)).bg(p.divider))
            .child(sync)
            .child(div().h(px(1.)).bg(p.divider))
            .child(effects)
    }

    /// The Audio tab's sections of sliders, after the Equalizer.
    fn audio_sections(&self) -> impl Iterator<Item = &EffectControls> {
        let shown = PanelTab::Audio.sections();
        self.settings_panel.effects.iter().filter(|controls| shown.contains(&controls.section))
    }

    /// Whether the audio track playing has one channel.
    fn plays_mono(&self, cx: &App) -> bool {
        let Some(player) = self.loaded_player(cx).map(|player| player.read(cx)) else {
            return false;
        };
        let track = player.audio_track().and_then(|id| player.tracks().get(id));
        track.is_some_and(|track| track.channels == Some(1))
    }

    /// "For “English 5.1” in this video."; `None` while no audio plays.
    fn audio_target(&self, cx: &App) -> Option<String> {
        let player = self.loaded_player(cx)?.read(cx);
        let label = player.tracks().label(player.audio_track()?)?;
        Some(format!("For “{label}” in this video."))
    }

    /// Shows the settings, the stereo mode in effect and the playing track's delay.
    fn sync_audio_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_panel.audio.defaults.sync(window, cx);
        self.settings_panel.audio.equalizer.sync(window, cx);
        for controls in self.audio_sections() {
            controls.sync(window, cx);
        }
        let index = stereo_index(cx);
        let controls = &self.settings_panel.audio;
        controls.stereo.update(cx, |dropdown, cx| dropdown.set_selected(index, cx));
        let player = self.loaded_player(cx).map(|player| player.read(cx));
        let playing = player.is_some_and(|player| player.audio_track().is_some());
        let delay = player.map_or_default(Player::audio_delay);
        controls.delay.update(cx, |row, cx| {
            row.set_disabled(!playing, cx);
            row.show(delay.as_millis() as f64 / MS_PER_SECOND, window, cx);
        });
    }

    /// Plays the test tone, saying which side sounds as it goes.
    fn test_stereo(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        player.update(cx, Player::play_test_tone);
        self.show_osd("Test tone: left", cx);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(test_tone::RIGHT_STARTS).await;
            // The window may be gone; nobody is left to tell then.
            let _ = this.update(cx, |this, cx| this.show_osd("Test tone: right", cx));
        })
        .detach();
    }
}

/// Stereo Mode's label, and Test at the right.
fn stereo_row(p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let test = Button::new("test-stereo-mode", "Test")
        .style(ButtonStyle::Subtle)
        .tooltip("Plays a tone on the left, then the right, at a moderate level", None::<&str>)
        .on_click(cx.listener(|this, _, _, cx| this.test_stereo(cx)));
    div()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(4.))
        .text_size(text_px(ROW_TEXT))
        .text_color(p.text_muted)
        .child("Stereo Mode")
        .child(test)
}

/// A line about the effect sections.
fn effects_note(text: &'static str, p: &Palette) -> Div {
    div()
        .pb(px(NOTE_GAP))
        .pl(px(4.))
        .text_size(text_px(SECTION_NOTE_TEXT))
        .text_color(p.text_muted)
        .child(text)
}

/// The line under the stereo mode: it changes the session, unlike the rows above it.
fn stereo_note(p: &Palette) -> Div {
    div()
        .pt(px(NOTE_GAP))
        .pl(px(4.))
        .text_size(text_px(SECTION_NOTE_TEXT))
        .text_color(p.text_muted)
        .mb(px(SECTION_NOTE_GAP))
        .child("For what plays until VLC quits; Save as Default keeps it.")
}

/// Stereo Mode's choices, as Settings → Audio has them.
fn stereo_choices() -> &'static [Choice] {
    match registry::find(STEREO_MODE).map(|def| def.control) {
        Some(Control::Dropdown(choices)) => choices,
        _ => &[],
    }
}

/// The position among the choices of the stereo mode in effect.
fn stereo_index(cx: &App) -> usize {
    let Some(Value::Choice(id)) = effects::value(cx, STEREO_MODE) else { return 0 };
    stereo_choices().iter().position(|choice| choice.id == id).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_mode_offers_the_designed_choices() {
        let labels: Vec<_> = stereo_choices().iter().map(|choice| choice.label).collect();
        assert_eq!(
            labels,
            ["Stereo", "Mono", "Left", "Right", "Reverse stereo", "Dolby Surround", "Headphones"]
        );
    }
}
