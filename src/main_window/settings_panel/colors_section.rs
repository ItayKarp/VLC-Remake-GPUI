//! The Video tab's Colors section (S7.4, `panels/video/colors/video_colors-expanded.png`): Color
//! Extraction and Color Threshold, each a color field and a switch, the threshold's Saturation
//! and Similarity; Negate Colors; Posterize; Gradient with its mode, Color and Cartoon; and Sepia
//! Intensity. It folds and has a title, Reset and an enable toggle like the other effect
//! sections (D23).
//!
//! What the mock leaves open (S7.4.2): Posterize gets a Levels slider under its switch (libVLC's
//! 2 to 256, 6 by default); Gradient's modes are libVLC's Gradient, Edge and Hough; and Sepia has
//! no switch, its slider saying Off at 0 like the picture's enhancements (S7.3). A filter's own
//! controls are disabled while its switch is off (S7.4.3); the colors can be picked before.
//!
//! A color field takes `#RRGGBB`, `#RGB` or a color name, and shows it as `#RRGGBB` with a
//! square of it beside. Anything else turns the field red with what it takes, and changes
//! nothing; leaving the field puts the color in effect back.

use gpui::{
    App, AppContext as _, Context, Div, Entity, SharedString, Subscription, Window, div,
    prelude::*, px, rgb,
};

use super::effect_section::{self, BOX_PADDING, DISABLED_OPACITY, EffectControls, TEXT};
use crate::effects::{self, VIDEO_COLORS};
use crate::main_window::MainWindow;
use crate::settings::registry::{self, Choice, Value, format_hex_color, hex_color};
use crate::theme::Palette;
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::metrics;
use crate::ui::text_input::{self, Look, TextInput, Validation};
use crate::ui::toggle::Toggle;

/// The section's settings.
const EXTRACT: &str = "video.colors.extract";
const THRESHOLD: &str = "video.colors.threshold";
const SATURATION: &str = "video.colors.threshold_saturation";
const SIMILARITY: &str = "video.colors.threshold_similarity";
const NEGATE: &str = "video.colors.negate";
const POSTERIZE: &str = "video.colors.posterize";
const LEVELS: &str = "video.colors.posterize_levels";
const GRADIENT: &str = "video.colors.gradient";
const MODE: &str = "video.colors.gradient_mode";
const GRADIENT_COLOR: &str = "video.colors.gradient_color";
const CARTOON: &str = "video.colors.cartoon";
const SEPIA: &str = "video.colors.sepia";
/// The two color fields: each one's setting, and its filter's switch beside it.
const COLOR_FIELDS: [(&str, &str); 2] =
    [("video.colors.extract_color", EXTRACT), ("video.colors.threshold_color", THRESHOLD)];
/// The rows' height, the color field's size, its square's size and corner, and the spaces.
const ROW_HEIGHT: f32 = 48.;
const FIELD_WIDTH: f32 = 116.;
const FIELD_HEIGHT: f32 = 32.;
const FIELD_PADDING: f32 = 12.;
const SWATCH: f32 = 20.;
const SWATCH_RADIUS: f32 = 2.;
const GAP: f32 = 8.;
/// The longest text a color field takes: a color's name or `#RRGGBB`.
const COLOR_MAX_LEN: usize = 16;
/// What a color field takes.
const COLOR_HELP: &str = "#RRGGBB, #RGB or a color name";

/// The section's controls that keep state.
pub(super) struct ColorsControls {
    sliders: EffectControls,
    colors: [Entity<TextInput>; 2],
    mode: Entity<Dropdown>,
    _events: Vec<Subscription>,
}

impl ColorsControls {
    /// The controls, changing the session when used.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let sliders = EffectControls::new(VIDEO_COLORS, window, cx);
        let colors = COLOR_FIELDS.map(|(key, _)| {
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .with_look(Look::Bare)
                    .with_text(in_effect(cx, key))
                    .with_label(label(key))
                    .with_max_len(COLOR_MAX_LEN)
            })
        });
        let mut events: Vec<_> = COLOR_FIELDS
            .iter()
            .zip(&colors)
            .map(|(&(key, _), field)| {
                cx.subscribe(field, move |_, field, event, cx| on_color(&field, key, event, cx))
            })
            .collect();
        let labels = modes().iter().map(|choice| SharedString::from(choice.label)).collect();
        let mode = cx.new(|cx| Dropdown::new(label(MODE), labels, 0, window, cx));
        events.push(cx.subscribe(&mode, |_, _, event: &dropdown::Changed, cx| {
            if let Some(choice) = modes().get(event.0) {
                effects::change(cx, &VIDEO_COLORS, MODE, Value::Choice(choice.id));
            }
        }));
        Self { sliders, colors, mode, _events: events }
    }

    /// Shows the values in effect, changed here or in Settings, and disables the controls whose
    /// switch is off.
    pub(super) fn sync(&self, window: &mut Window, cx: &mut Context<MainWindow>) {
        self.sliders.sync(window, cx);
        for (&(key, _), field) in COLOR_FIELDS.iter().zip(&self.colors) {
            let text = in_effect(cx, key);
            let input = field.read(cx);
            if !input.is_focused(window) && !input.is_invalid() && input.text() != text {
                field.update(cx, |field, cx| field.set_text(text, cx));
            }
        }
        let mode = match effects::value(cx, MODE) {
            Some(Value::Choice(id)) => modes().iter().position(|choice| choice.id == id),
            _ => None,
        };
        let live = effect_section::is_live(cx, &VIDEO_COLORS, MODE);
        self.mode.update(cx, |dropdown, cx| {
            dropdown.set_selected(mode.unwrap_or_default(), cx);
            dropdown.set_disabled(!live, cx);
        });
    }

    /// The section: its title row, then its box of filters, each set apart by a line.
    pub(super) fn render(&self, p: &Palette, cx: &App) -> Div {
        let content = effect_section::is_expanded(cx, &VIDEO_COLORS).then(|| {
            let groups = [
                vec![self.color_row(0, p, cx)],
                vec![
                    self.color_row(1, p, cx),
                    self.slider(SATURATION, p, cx),
                    self.slider(SIMILARITY, p, cx),
                ],
                vec![toggle_row(NEGATE, p, cx)],
                vec![toggle_row(POSTERIZE, p, cx), self.slider(LEVELS, p, cx)],
                vec![
                    toggle_row(GRADIENT, p, cx),
                    div().pt(px(GAP)).child(self.mode.clone()),
                    toggle_row(GRADIENT_COLOR, p, cx),
                    toggle_row(CARTOON, p, cx),
                ],
                vec![self.slider(SEPIA, p, cx)],
            ];
            let body = div().flex().flex_col().px(px(BOX_PADDING)).pb(px(BOX_PADDING));
            groups.into_iter().enumerate().fold(body, |body, (index, rows)| {
                let rows = div().flex().flex_col().py(px(GAP)).children(rows);
                body.when(index > 0, |body| body.child(div().h(px(1.)).bg(p.divider))).child(rows)
            })
        });
        effect_section::folding(&VIDEO_COLORS, content, p, cx)
    }

    /// The slider of `key` under its label.
    fn slider(&self, key: &str, p: &Palette, cx: &App) -> Div {
        let live = effect_section::is_live(cx, &VIDEO_COLORS, key);
        self.sliders
            .slider(key)
            .map_or_else(div, |(label, slider)| effect_section::slider_row(label, slider, live, p))
    }

    /// A color field's row: its filter's name, the field with its square, and the filter's
    /// switch; with what the field takes under it while it holds something else.
    fn color_row(&self, index: usize, p: &Palette, cx: &App) -> Div {
        let (key, filter) = COLOR_FIELDS[index];
        let field = &self.colors[index];
        let shown = hex_color(field.read(cx).text()).or_else(|| hex_color(&in_effect(cx, key)));
        let square = div()
            .flex_none()
            .size(px(SWATCH))
            .rounded(px(SWATCH_RADIUS))
            .border_1()
            .border_color(p.divider)
            .when_some(shown, |square, color| square.bg(rgb(color)));
        let color = div()
            .w(px(FIELD_WIDTH))
            .h(px(FIELD_HEIGHT))
            .flex()
            .items_center()
            .gap(px(GAP))
            .px(px(FIELD_PADDING))
            .rounded(px(metrics::RADIUS))
            .bg(p.well)
            .child(div().flex_1().min_w_0().text_color(p.text).child(field.clone()))
            .child(square);
        let controls = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(GAP))
            .child(color)
            .child(switch(filter, true, cx));
        let error = field
            .read(cx)
            .is_invalid()
            .then(|| div().pb(px(GAP)).text_size(px(TEXT)).text_color(p.danger).child(COLOR_HELP));
        div().flex().flex_col().child(labelled(filter, true, p).child(controls)).children(error)
    }
}

/// A switch's row: its label, and the switch at the right.
fn toggle_row(key: &'static str, p: &Palette, cx: &App) -> Div {
    let live = effect_section::is_live(cx, &VIDEO_COLORS, key);
    labelled(key, live, p).child(switch(key, live, cx))
}

/// A row with `key`'s label at the left, dimmed while its control isn't `live`.
fn labelled(key: &str, live: bool, p: &Palette) -> Div {
    let label = div().flex_1().min_w_0().when(!live, |label| label.opacity(DISABLED_OPACITY));
    div()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .gap(px(GAP))
        .text_size(px(TEXT))
        .text_color(p.text_muted)
        .child(label.child(self::label(key)))
}

/// The switch of `key`, which changes the session.
fn switch(key: &'static str, live: bool, cx: &App) -> Toggle {
    let on = effects::value(cx, key) == Some(Value::Bool(true));
    Toggle::new(SharedString::from(format!("colors-{key}")), on, label(key))
        .disabled(!live)
        .on_change(move |on, _, cx| effects::change(cx, &VIDEO_COLORS, key, Value::Bool(*on)))
}

/// Takes a typed color, or says what one looks like.
fn on_color(
    field: &Entity<TextInput>,
    key: &'static str,
    event: &text_input::Event,
    cx: &mut Context<MainWindow>,
) {
    let typed = hex_color(field.read(cx).text()).map(format_hex_color);
    match (event, typed) {
        (text_input::Event::Changed, _) => {
            field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
        }
        (text_input::Event::Submitted | text_input::Event::Blurred, Some(text)) => {
            if in_effect(cx, key) != text {
                effects::change(cx, &VIDEO_COLORS, key, Value::Text(text.clone()));
            }
            field.update(cx, |field, cx| field.set_text(text, cx));
        }
        (text_input::Event::Submitted, None) => field.update(cx, |field, cx| {
            field.set_validation(Validation::Error(COLOR_HELP.into()), cx);
        }),
        (text_input::Event::Blurred, None) => {
            let text = in_effect(cx, key);
            field.update(cx, |field, cx| {
                field.set_text(text, cx);
                field.set_validation(Validation::None, cx);
            });
        }
    }
}

/// The color in effect for `key`, as `#RRGGBB`.
fn in_effect(cx: &App, key: &str) -> String {
    match effects::value(cx, key) {
        Some(Value::Text(text)) => text,
        _ => String::new(),
    }
}

/// The label of the setting `key`.
fn label(key: &str) -> &'static str {
    registry::find(key).map_or("", |def| def.label)
}

/// Gradient's modes.
fn modes() -> &'static [Choice] {
    registry::find(MODE).map_or(&[], |def| def.choices())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::registry::Control;

    #[test]
    fn every_control_is_a_setting_of_its_kind() {
        for key in [EXTRACT, THRESHOLD, NEGATE, POSTERIZE, GRADIENT, GRADIENT_COLOR, CARTOON] {
            let def = registry::find(key).expect("a switch is a setting");
            assert!(matches!(def.control, Control::Toggle), "{key}");
        }
        for (key, _) in COLOR_FIELDS {
            let def = registry::find(key).expect("a color is a setting");
            assert!(matches!(def.control, Control::HexColor), "{key}");
        }
        for key in [SATURATION, SIMILARITY, LEVELS, SEPIA] {
            let def = registry::find(key).expect("a slider is a setting");
            assert!(matches!(def.control, Control::Slider { .. }), "{key}");
        }
        assert_eq!(modes().len(), 3);
    }
}
