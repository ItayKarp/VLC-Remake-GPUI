//! The Video tab's Defaults: angles typed into Rotate, and the dropdown rows, used in a window
//! playing the mock's clip.

use gpui::{Focusable, TestAppContext, VisualTestContext};

use super::*;
use crate::main_window::now_playing::tests::draw;
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{
    field_text, leave, open_tab, press_near, press_on, submit, type_over,
};
use crate::main_window::tests::main_view;

fn rotate_field(cx: &mut VisualTestContext) -> Entity<TextInput> {
    main_view(cx).read_with(cx, |main, _| main.settings_panel.video_defaults.rotate.field().clone())
}

fn rotation(cx: &mut VisualTestContext) -> Rotation {
    cx.update(|_, cx| effects::effective(cx).prefs.video.rotation)
}

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

#[gpui::test]
fn a_typed_angle_snaps_to_a_quarter_turn(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    let field = rotate_field(cx);
    assert_eq!(field_text(cx, &field), ("0 deg".into(), false));
    submit(cx, &field, "-80");
    assert_eq!(rotation(cx), Rotation::ThreeQuarters);
    assert_eq!(field_text(cx, &field), ("270 deg".into(), false));
    submit(cx, &field, "180°");
    assert_eq!(rotation(cx), Rotation::Half);
}

#[gpui::test]
fn text_that_isnt_an_angle_is_refused_and_put_back(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    let field = rotate_field(cx);
    submit(cx, &field, "sideways");
    assert_eq!(field_text(cx, &field), ("sideways".into(), true));
    assert_eq!(rotation(cx), Rotation::Upright);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(field_text(cx, &field), ("0 deg".into(), false), "Escape puts it back");
    assert!(main_view(cx).read_with(cx, |main, _| main.settings_panel.is_open()));
    type_over(cx, &field, "upside down");
    leave(cx);
    assert_eq!(field_text(cx, &field), ("0 deg".into(), false), "so does leaving");
}

#[gpui::test]
fn a_dropdown_holds_its_choice_for_the_session(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    let (dropdown, choices) = main_view(cx).read_with(cx, |main, _| {
        let rows = &main.settings_panel.video_defaults.dropdowns;
        let row = rows.iter().find(|row| row.key == ASPECT_RATIO).expect("an Aspect Ratio row");
        (row.dropdown.clone(), row.choices)
    });
    press_on(cx, &dropdown, "end");
    let last = choices.last().expect("choices").id;
    let held = cx.update(|_, cx| effects::value(cx, ASPECT_RATIO));
    assert_eq!(held, Some(Value::Choice(last)));
    let saved = cx.update(|_, cx| crate::settings::get(cx).prefs.video.aspect_ratio);
    assert_eq!(saved, AspectRatio::default(), "Settings keep their own");
    assert!(cx.update(|_, cx| VIDEO_DEFAULTS.iter().any(|key| effects::is_changed(cx, key))));
    effects_reset(cx);
    assert_eq!(dropdown.read_with(cx, |dropdown, _| dropdown.selected()), 0);
}

/// Puts the Defaults back on Settings' values, as Reset does, and draws.
fn effects_reset(cx: &mut VisualTestContext) {
    cx.update(|_, cx| effects::reset(cx, VIDEO_DEFAULTS.iter().copied()));
    draw(cx);
}

#[gpui::test]
fn flip_and_reset_answer_the_keyboard(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    let rotate = rotate_field(cx);
    let handle = rotate.read_with(cx, Focusable::focus_handle);
    press_near(cx, &handle, -1, "right");
    let flipped = cx.update(|_, cx| effects::value(cx, FLIP));
    assert_ne!(flipped, cx.update(|_, cx| effects::default_value(cx, FLIP)), "Flip moved");
    press_near(cx, &handle, -2, "space");
    let flip = cx.update(|_, cx| effects::value(cx, FLIP));
    assert_eq!(flip, cx.update(|_, cx| effects::default_value(cx, FLIP)), "Reset put it back");
}
