//! The Playback tab's Audio Visualizer row (S7.8.1, `controls-playback.png`,
//! `panels/audio/visualizer`): a dropdown of Disabled, Spectrometer, Scope, Spectrum and Waveform,
//! each but Disabled with its glyph. It is Settings → Audio's "Audio visualizer", kept across
//! launches, as the timeline toggle above it is Settings → Interface's.

use gpui::{Context, Div, Entity, SharedString, Subscription, Window, div, prelude::*, px};

use crate::assets::visualizer_glyph;
use crate::engine::audio::visualizer::Visualizer;
use crate::main_window::MainWindow;
use crate::settings;
use crate::theme::Palette;
use crate::ui::dropdown::{self, Dropdown};

/// The row's label, height and text size, and the dropdown's width.
const LABEL: &str = "Audio Visualizer";
const ROW_HEIGHT: f32 = 68.;
const ROW_TEXT: f32 = 14.;
const DROPDOWN_WIDTH: f32 = 220.;

/// The visualizer's dropdown.
pub(in crate::main_window) struct VisualizerControl {
    dropdown: Entity<Dropdown>,
    _picked: Subscription,
}

impl VisualizerControl {
    /// The dropdown, showing the setting.
    pub(in crate::main_window) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let labels = Visualizer::ALL.iter().map(|choice| SharedString::from(choice.label()));
        let glyphs = Visualizer::ALL.into_iter().map(visualizer_glyph).collect();
        let selected = index(cx);
        let dropdown = cx.new(|cx| {
            let mut dropdown = Dropdown::new(LABEL, labels.collect(), selected, window, cx);
            dropdown.set_glyphs(glyphs, cx);
            dropdown
        });
        let picked = cx.subscribe(&dropdown, |_, _, event: &dropdown::Changed, cx| {
            if let Some(&visualizer) = Visualizer::ALL.get(event.0) {
                settings::update(cx, |s| s.prefs.audio.visualizer = visualizer);
            }
        });
        Self { dropdown, _picked: picked }
    }

    /// The row: its label, and the dropdown showing the setting.
    pub(super) fn render(&self, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
        let selected = index(cx);
        self.dropdown.update(cx, |dropdown, cx| dropdown.set_selected(selected, cx));
        div()
            .h(px(ROW_HEIGHT))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .pl(px(4.))
            .text_size(px(ROW_TEXT))
            .text_color(p.text_muted)
            .child(LABEL)
            .child(div().w(px(DROPDOWN_WIDTH)).flex_none().child(self.dropdown.clone()))
    }
}

/// The setting's place in the dropdown.
fn index(cx: &gpui::App) -> usize {
    let visualizer = settings::get(cx).prefs.audio.visualizer;
    Visualizer::ALL.iter().position(|choice| *choice == visualizer).unwrap_or_default()
}
