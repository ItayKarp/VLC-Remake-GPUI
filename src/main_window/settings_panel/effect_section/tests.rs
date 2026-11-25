//! An effect section: remembered open, its sliders moved from the keyboard and put back, and
//! every section of the Video and Audio tabs drawn open.

use gpui::{Focusable as _, TestAppContext, VisualTestContext};

use super::*;
use crate::effects::{
    AUDIO_COMPRESSOR, AUDIO_EQUALIZER, AUDIO_SPATIALIZER, AUDIO_STEREO_WIDENER, VIDEO_COLORS,
    VIDEO_CROP, VIDEO_ESSENTIAL,
};
use crate::main_window::now_playing::tests::draw;
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{expand, open_tab, press_near, show_tab};
use crate::main_window::tests::main_view;

const BRIGHTNESS: &str = "video.picture.brightness";

fn essential_slider(cx: &mut VisualTestContext) -> Entity<Slider> {
    main_view(cx).read_with(cx, |main, _| {
        let controls = main.settings_panel.effects.iter();
        let essential = controls.into_iter().find(|c| c.section == VIDEO_ESSENTIAL);
        essential.and_then(|c| c.slider(BRIGHTNESS)).expect("a brightness slider").1.clone()
    })
}

fn brightness(cx: &mut VisualTestContext) -> Option<Value> {
    cx.update(|_, cx| effects::value(cx, BRIGHTNESS))
}

#[gpui::test]
fn a_section_is_remembered_open_by_its_id(cx: &mut TestAppContext) {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        assert!(!is_expanded(cx, &VIDEO_ESSENTIAL));
        set_expanded(cx, VIDEO_ESSENTIAL.id, true);
        set_expanded(cx, VIDEO_ESSENTIAL.id, true);
        assert!(is_expanded(cx, &VIDEO_ESSENTIAL));
        assert_eq!(settings::get(cx).local.expanded_effects.len(), 1);
        set_expanded(cx, VIDEO_ESSENTIAL.id, false);
        assert!(!is_expanded(cx, &VIDEO_ESSENTIAL));
    });
}

#[gpui::test]
fn a_slider_moved_changes_the_session_and_turns_its_section_on(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    expand(cx, &[VIDEO_ESSENTIAL]);
    let before = brightness(cx);
    let slider = essential_slider(cx);
    cx.update(|window, cx| slider.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);
    cx.simulate_keystrokes("end");
    draw(cx);
    assert_ne!(brightness(cx), before);
    let on = cx.update(|_, cx| effects::value(cx, VIDEO_ESSENTIAL.toggle));
    assert_eq!(on, Some(Value::Bool(true)));
    assert!(cx.update(|_, cx| effects::any_changed(cx, &VIDEO_ESSENTIAL)));
    // A double-click puts the slider back on its default.
    slider.update(cx, |_, cx| cx.emit(slider::Reset));
    draw(cx);
    assert_eq!(brightness(cx), before);
}

#[gpui::test]
fn every_section_draws_open(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    let all = [
        VIDEO_CROP,
        VIDEO_ESSENTIAL,
        VIDEO_COLORS,
        AUDIO_EQUALIZER,
        AUDIO_COMPRESSOR,
        AUDIO_SPATIALIZER,
        AUDIO_STEREO_WIDENER,
    ];
    expand(cx, &all);
    show_tab(cx, PanelTab::Audio);
    for section in all {
        assert!(cx.update(|_, cx| is_expanded(cx, &section)), "{}", section.id);
    }
}

#[gpui::test]
fn the_title_row_and_fold_answer_the_keyboard(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Video);
    expand(cx, &[VIDEO_ESSENTIAL]);
    let first = main_view(cx).read_with(cx, |main, cx| {
        let essential = main.settings_panel.effects.iter().find(|c| c.section == VIDEO_ESSENTIAL);
        let (_, _, slider) = &essential.expect("the Essential section").sliders[0];
        slider.read(cx).focus_handle(cx)
    });
    let enabled =
        |cx: &mut VisualTestContext| cx.update(|_, cx| effects::value(cx, VIDEO_ESSENTIAL.toggle));
    press_near(cx, &first, -2, "space");
    assert_eq!(enabled(cx), Some(Value::Bool(true)), "the toggle turned the section on");
    press_near(cx, &first, -3, "space");
    assert_eq!(enabled(cx), Some(Value::Bool(false)), "Reset put it back");
    press_near(cx, &first, -1, "space");
    assert!(!cx.update(|_, cx| is_expanded(cx, &VIDEO_ESSENTIAL)), "the header folded it");
}
