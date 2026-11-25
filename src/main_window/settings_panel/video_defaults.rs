//! The Video tab's Defaults (S7.2, `controls-video_settings.png`): **Flip**, **Rotate**,
//! **Zoom**, **Resolution**, **Aspect Ratio**, **Crop** and **HDR** (S7.5). Like the effect sections they change
//! what plays for the session (D24), over Settings → Video's Framing and Settings → Playback's
//! preferred quality; Reset beside the heading lets them follow Settings again, and Save as
//! Default keeps them there.
//!
//! Rotate is a field, as designed ("0 deg"): any angle typed snaps to the nearest quarter turn
//! (−90 is 270). Flip and Rotate reopen the video where it is; the rest the app draws at once.
//! Aspect Ratio's Custom shows a Custom Ratio field under it (`custom_ratio.rs`).
//! Resolution only reaches streams that offer several, from the next one opened. HDR reopens
//! HDR video where it is, with a line under it saying what plays and how it shows. Render
//! (`render_row.rs`, S8.11) comes last: where media plays, here or on a cast device.

use gpui::{
    App, AppContext as _, Context, Div, Entity, KeyDownEvent, SharedString, Subscription, Window,
    div, prelude::*, px,
};

use super::custom_ratio::CustomRatioField;
use super::render_row::RenderControl;
use super::synced_field::SyncedField;
use crate::effects::{self, VIDEO_DEFAULTS};
use crate::engine::transform::Rotation;
use crate::i18n;
use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::settings::registry::{self, Choice, Control, SettingChoice as _, Value};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::segmented::Segmented;
use crate::ui::text_input::{self, TextInput, Validation};
use crate::video::framing::AspectRatio;

/// The settings behind the rows.
const FLIP: &str = "video.flip";
const ROTATION: &str = "video.rotation";
/// The dropdown rows: each setting's key and the row's label.
const ASPECT_RATIO: &str = "video.aspect_ratio";
const DROPDOWNS: [(&str, &str); 5] = [
    ("video.zoom", tr_noop!("Zoom")),
    ("player.video_quality", tr_noop!("Resolution")),
    (ASPECT_RATIO, tr_noop!("Aspect Ratio")),
    ("video.crop", tr_noop!("Crop")),
    (HDR, tr_noop!("HDR")),
];
const HDR: &str = "video.hdr";
/// The rows' height, text size and control width, and the space under Flip's label.
const ROW_HEIGHT: f32 = 68.;
const ROW_TEXT: f32 = 14.;
pub(super) const CONTROL_WIDTH: f32 = 220.;
const ROTATE_WIDTH: f32 = 136.;
const LABEL_GAP: f32 = 12.;
/// What the heading says the rows change.
const NOTE: &str = tr_noop!("For what plays until VLC quits; Save as Default keeps them.");
/// Why typed text isn't an angle.
const NOT_AN_ANGLE: &str = tr_noop!("Degrees, such as 90 or -90");

/// One dropdown row: its setting, the setting's choices, and the dropdown.
struct DropdownRow {
    key: &'static str,
    label: &'static str,
    choices: &'static [Choice],
    dropdown: Entity<Dropdown>,
}

/// The Defaults rows' controls that keep state.
pub(super) struct DefaultsControls {
    /// The Rotate field.
    pub(super) rotate: SyncedField,
    /// The Custom Ratio field, shown while Aspect Ratio is Custom.
    pub(super) custom_ratio: CustomRatioField,
    dropdowns: Vec<DropdownRow>,
    /// The Render row (S8.11).
    pub(super) render: RenderControl,
    _events: Vec<Subscription>,
}

impl DefaultsControls {
    /// The controls, changing the session when used.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let rotate = cx.new(|cx| {
            TextInput::new(window, cx).with_text(rotate_text(cx)).with_label(tr!("Rotate"))
        });
        let mut events = vec![cx.subscribe(&rotate, |_, field, event, cx| {
            MainWindow::on_rotate(&field, event, cx);
        })];
        let mut dropdowns = Vec::new();
        for (key, label) in DROPDOWNS {
            let Some(Control::Dropdown(choices)) = registry::find(key).map(|def| def.control)
            else {
                tracing::warn!("settings panel: {key} isn't a dropdown setting");
                continue;
            };
            let labels =
                choices.iter().map(|choice| SharedString::from(i18n::text(choice.label))).collect();
            let dropdown = cx.new(|cx| Dropdown::new(i18n::text(label), labels, 0, window, cx));
            events.push(cx.subscribe(&dropdown, move |_, _, event: &dropdown::Changed, cx| {
                pick(cx, key, choices, event.0);
            }));
            dropdowns.push(DropdownRow { key, label, choices, dropdown });
        }
        let custom_ratio = CustomRatioField::new(window, cx);
        let render = RenderControl::new(window, cx);
        Self { rotate: SyncedField::new(rotate), custom_ratio, dropdowns, render, _events: events }
    }

    /// Shows the values in effect, changed here or in Settings.
    pub(super) fn sync(&mut self, window: &Window, cx: &mut Context<MainWindow>) {
        for row in &self.dropdowns {
            let index = chosen(cx, row.key, row.choices);
            row.dropdown.update(cx, |dropdown, cx| dropdown.set_selected(index, cx));
        }
        self.custom_ratio.sync(window, cx);
        self.rotate.sync(rotate_text(cx), window, cx);
    }
}

impl MainWindow {
    /// The Defaults section.
    pub(super) fn render_video_defaults(
        &self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let controls = &self.settings_panel.video_defaults;
        let hdr_note = self.hdr_note(window, cx);
        let rotate = div()
            .w(px(ROTATE_WIDTH))
            .on_key_down(cx.listener(Self::on_rotate_key))
            .child(controls.rotate.field().clone());
        let custom = effects::effective(cx).prefs.video.aspect_ratio == AspectRatio::Custom;
        let dropdowns = controls.dropdowns.iter().flat_map(|row| {
            let control = div().w(px(CONTROL_WIDTH)).child(row.dropdown.clone());
            let dropdown = labelled(i18n::text(row.label), control, p);
            let field = (custom && row.key == ASPECT_RATIO).then(|| {
                let field = div().w(px(CONTROL_WIDTH)).child(controls.custom_ratio.field());
                labelled(tr!("Custom Ratio"), field, p)
            });
            let note = (row.key == HDR).then(|| hdr_note.clone().map(|text| note(text, p)));
            std::iter::once(dropdown).chain(field).chain(note.flatten())
        });
        div()
            .flex_row_start()
            .flex_col()
            .child(heading(p, cx))
            .child(flip_row(p, cx))
            .child(labelled(tr!("Rotate"), rotate, p))
            .children(dropdowns)
            .child(self.render_render_row(p, cx))
    }

    fn on_rotate(field: &Entity<TextInput>, event: &text_input::Event, cx: &mut Context<Self>) {
        let parsed = parse_angle(field.read(cx).text());
        match (event, parsed) {
            (text_input::Event::Changed, _) => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            (text_input::Event::Submitted | text_input::Event::Blurred, Some(rotation)) => {
                effects::set(cx, ROTATION, Value::Choice(rotation.choice_id()));
                let text = format_angle(rotation);
                field.update(cx, |field, cx| field.set_text(text, cx));
            }
            (text_input::Event::Submitted, None) => field.update(cx, |field, cx| {
                field.set_validation(Validation::Error(i18n::text(NOT_AN_ANGLE).into()), cx);
            }),
            (text_input::Event::Blurred, None) => {
                let text = rotate_text(cx);
                field.update(cx, |field, cx| {
                    field.set_text(text, cx);
                    field.set_validation(Validation::None, cx);
                });
            }
        }
    }

    /// Escape in the Rotate field puts the angle in effect back before it closes the panel.
    fn on_rotate_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
            return;
        }
        let text = rotate_text(cx);
        let field = self.settings_panel.video_defaults.rotate.field().clone();
        if field.read(cx).text() != text || field.read(cx).is_invalid() {
            field.update(cx, |field, cx| {
                field.set_text(text, cx);
                field.set_validation(Validation::None, cx);
            });
            cx.stop_propagation();
        }
    }
}

/// The heading, the line under it, and Reset, dimmed while the panel changed none of the rows.
fn heading(p: &Palette, cx: &App) -> Div {
    let reset = Button::new("reset-video-defaults", tr!("Reset"))
        .style(ButtonStyle::Ghost)
        .disabled(!VIDEO_DEFAULTS.iter().any(|key| effects::is_changed(cx, key)))
        .tooltip(tr!("Put these back on Settings' values"), None::<&str>)
        .on_click(|_, _, cx| effects::reset(cx, VIDEO_DEFAULTS.iter().copied()));
    let title = div()
        .flex_row_start()
        .items_center()
        .justify_between()
        .child(
            div().text_size(text_px(super::HEADING_TEXT)).text_color(p.text).child(tr!("Defaults")),
        )
        .child(reset);
    let note = div()
        .pt(px(super::SECTION_NOTE_GAP))
        .text_size(text_px(super::SECTION_NOTE_TEXT))
        .text_color(p.text_muted)
        .child(i18n::text(NOTE));
    div().flex_row_start().flex_col().pb(px(super::SECTION_NOTE_GAP)).child(title).child(note)
}

/// A line under a row, about what it does to the media playing.
pub(super) fn note(text: String, p: &Palette) -> Div {
    div()
        .pb(px(LABEL_GAP))
        .text_size(text_px(super::SECTION_NOTE_TEXT))
        .text_color(p.text_muted)
        .child(text)
}

/// Flip: its label over a segmented control the panel's width.
fn flip_row(p: &Palette, cx: &App) -> Div {
    let choices = registry::find(FLIP).map(|def| def.control);
    let Some(Control::Segmented(choices)) = choices else { return div() };
    let selected = chosen(cx, FLIP, choices);
    let control = Segmented::new(
        "video-flip",
        tr!("Flip"),
        choices.iter().map(|c| i18n::text(c.label)),
        selected,
    )
    .on_select(move |index, _, cx| pick(cx, FLIP, choices, *index));
    div()
        .flex_row_start()
        .flex_col()
        .gap(px(LABEL_GAP))
        .pt(px(LABEL_GAP))
        .text_size(text_px(ROW_TEXT))
        .text_color(p.text_muted)
        .child(tr!("Flip"))
        .child(control)
}

/// A row: the label, and `control` at the right.
pub(super) fn labelled(label: &'static str, control: impl IntoElement, p: &Palette) -> Div {
    div()
        .h(px(ROW_HEIGHT))
        .flex_row_start()
        .items_center()
        .justify_between()
        .text_size(text_px(ROW_TEXT))
        .text_color(p.text_muted)
        .child(label)
        .child(div().flex_none().child(control))
}

/// Holds the choice at `index` of `key`'s `choices` for the session.
fn pick(cx: &mut App, key: &'static str, choices: &'static [Choice], index: usize) {
    if let Some(choice) = choices.get(index) {
        effects::set(cx, key, Value::Choice(choice.id));
    }
}

/// The position among `choices` of `key`'s value in effect.
fn chosen(cx: &App, key: &str, choices: &[Choice]) -> usize {
    let Some(Value::Choice(id)) = effects::value(cx, key) else { return 0 };
    choices.iter().position(|choice| choice.id == id).unwrap_or_default()
}

/// The Rotate field's text for the rotation in effect.
fn rotate_text(cx: &App) -> String {
    format_angle(effects::effective(cx).prefs.video.rotation)
}

/// "90 deg".
fn format_angle(rotation: Rotation) -> String {
    format!("{} deg", rotation.degrees())
}

/// A typed angle ("90", "-90 deg", "180°") as the nearest quarter turn.
fn parse_angle(text: &str) -> Option<Rotation> {
    let text = text.trim().to_lowercase();
    let number = ["degrees", "degree", "deg", "°"]
        .iter()
        .find_map(|unit| text.strip_suffix(unit))
        .unwrap_or(&text)
        .trim();
    number.parse::<f64>().ok().filter(|angle| angle.is_finite()).map(Rotation::nearest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_read_with_or_without_a_unit_and_snap() {
        assert_eq!(parse_angle("90"), Some(Rotation::Quarter));
        assert_eq!(parse_angle(" -90 deg "), Some(Rotation::ThreeQuarters));
        assert_eq!(parse_angle("180°"), Some(Rotation::Half));
        assert_eq!(parse_angle("100 Degrees"), Some(Rotation::Quarter));
        assert_eq!(parse_angle("sideways"), None);
        assert_eq!(parse_angle("inf"), None);
        assert_eq!(format_angle(Rotation::ThreeQuarters), "270 deg");
    }

    #[test]
    fn every_row_is_a_setting_of_its_kind() {
        for (key, _) in DROPDOWNS {
            let control = registry::find(key).map(|def| def.control);
            assert!(matches!(control, Some(Control::Dropdown(_))), "{key}");
        }
        let flip = registry::find(FLIP).map(|def| def.control);
        assert!(matches!(flip, Some(Control::Segmented(_))));
        assert!(registry::find(ROTATION).is_some());
    }
}
