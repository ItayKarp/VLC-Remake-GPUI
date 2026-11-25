//! The Colors section: every control a setting of its kind, colors typed by name or hex, and
//! the gradient's mode, used in a window playing the mock's clip.

use gpui::{Focusable, TestAppContext, VisualTestContext};

use super::*;
use crate::main_window::now_playing::tests::draw;
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{
    expand, field_text, leave, open_tab, press_near, press_on, submit, type_over,
};
use crate::main_window::tests::main_view;
use crate::settings::registry::Control;

/// The Video tab with the Colors section open.
fn open_colors(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    expand(cx, &[VIDEO_COLORS]);
    cx
}

fn color_field(cx: &mut VisualTestContext, index: usize) -> Entity<TextInput> {
    main_view(cx).read_with(cx, |main, _| main.settings_panel.colors.colors[index].field().clone())
}

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

#[gpui::test]
fn a_color_takes_a_name_or_hex_and_shows_hex(cx: &mut TestAppContext) {
    let cx = open_colors(cx);
    let extract = color_field(cx, 0);
    submit(cx, &extract, "red");
    assert_eq!(field_text(cx, &extract).0, "#FF0000");
    assert_eq!(cx.update(|_, cx| in_effect(cx, COLOR_FIELDS[0].0)), "#FF0000");
    submit(cx, &extract, "#0f0");
    assert_eq!(cx.update(|_, cx| in_effect(cx, COLOR_FIELDS[0].0)), "#00FF00");
}

#[gpui::test]
fn text_that_isnt_a_color_is_refused_until_left(cx: &mut TestAppContext) {
    let cx = open_colors(cx);
    let threshold = color_field(cx, 1);
    let before = cx.update(|_, cx| in_effect(cx, COLOR_FIELDS[1].0));
    submit(cx, &threshold, "plaid");
    assert_eq!(field_text(cx, &threshold), ("plaid".into(), true));
    assert_eq!(cx.update(|_, cx| in_effect(cx, COLOR_FIELDS[1].0)), before);
    type_over(cx, &threshold, "still plaid");
    leave(cx);
    assert_eq!(field_text(cx, &threshold), (before, false), "leaving puts it back");
}

#[gpui::test]
fn the_gradient_mode_waits_for_its_switch(cx: &mut TestAppContext) {
    let cx = open_colors(cx);
    let mode = main_view(cx).read_with(cx, |main, _| main.settings_panel.colors.mode.clone());
    cx.update(|_, cx| effects::change(cx, &VIDEO_COLORS, GRADIENT, Value::Bool(true)));
    draw(cx);
    press_on(cx, &mode, "end");
    let last = modes().last().expect("modes").id;
    assert_eq!(cx.update(|_, cx| effects::value(cx, MODE)), Some(Value::Choice(last)));
    assert_eq!(mode.read_with(cx, |mode, _| mode.selected()), modes().len() - 1);
}

#[gpui::test]
fn a_filters_switch_answers_the_keyboard(cx: &mut TestAppContext) {
    let cx = open_colors(cx);
    let extract = color_field(cx, 0);
    let handle = extract.read_with(cx, Focusable::focus_handle);
    press_near(cx, &handle, 1, "space");
    assert_eq!(cx.update(|_, cx| effects::value(cx, EXTRACT)), Some(Value::Bool(true)));
}
