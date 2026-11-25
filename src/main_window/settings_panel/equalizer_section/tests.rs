//! The Equalizer section: presets picked, bands typed, "Save as preset…" and the clipping
//! warning, first as rules, then used in a window playing the mock's clip.

use gpui::{Focusable, TestAppContext, VisualTestContext};

use super::*;
use crate::engine::audio::equalizer::Preset;
use crate::main_window::now_playing::tests::draw;
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{
    expand, field_text, open_tab, press_near, press_on, submit,
};
use crate::main_window::tests::main_view;

/// The Audio tab with the Equalizer open.
fn open_equalizer(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let (cx, _playing) = open_tab(cx, PanelTab::Audio);
    expand(cx, &[AUDIO_EQUALIZER]);
    cx
}

fn picked(cx: &mut VisualTestContext) -> EqualizerPick {
    cx.update(|_, cx| pick(cx))
}

/// The name field "Save as preset…" opened, if open.
fn naming(cx: &mut VisualTestContext) -> Option<Entity<TextInput>> {
    main_view(cx).read_with(cx, |main, _| {
        main.settings_panel.audio.equalizer.naming.as_ref().map(|(field, _)| field.clone())
    })
}

fn open_naming(cx: &mut VisualTestContext) -> Entity<TextInput> {
    main_view(cx).update_in(cx, MainWindow::open_naming);
    draw(cx);
    naming(cx).expect("the name field opens")
}

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

#[gpui::test]
fn a_preset_picked_from_the_list_plays(cx: &mut TestAppContext) {
    let cx = open_equalizer(cx);
    let presets =
        main_view(cx).read_with(cx, |main, _| main.settings_panel.audio.equalizer.presets.clone());
    press_on(cx, &presets, "r");
    assert_eq!(picked(cx), EqualizerPick::Preset(Preset::Reggae));
    press_on(cx, &presets, "home");
    assert_eq!(picked(cx), EqualizerPick::None, "None turns it off");
}

#[gpui::test]
fn a_typed_gain_moves_its_band_and_says_custom(cx: &mut TestAppContext) {
    let cx = open_equalizer(cx);
    let preamp = main_view(cx).read_with(cx, |main, cx| {
        main.settings_panel.audio.equalizer.gains[0].read(cx).field().clone()
    });
    submit(cx, &preamp, "15");
    assert_eq!(picked(cx), EqualizerPick::Preset(Preset::Custom));
    let gains = cx.update(|_, cx| effects::effective(cx).prefs.audio.equalizer_gains());
    assert!((gains.preamp - 15.).abs() < f32::EPSILON);
    cx.update(|_, cx| set_gain(1, 10., cx));
    draw(cx);
    assert!(cx.update(|_, cx| may_clip(&effects::effective(cx).prefs.audio.equalizer_gains())));
}

#[gpui::test]
fn save_as_preset_names_the_gains_in_use(cx: &mut TestAppContext) {
    let cx = open_equalizer(cx);
    cx.update(|_, cx| set_gain(3, 4., cx));
    let field = open_naming(cx);
    assert!(cx.update(|window, cx| field.read(cx).is_focused(window)));
    submit(cx, &field, "  ");
    assert!(field_text(cx, &field).1, "an empty name is refused");
    assert!(naming(cx).is_some());
    submit(cx, &field, "Late night");
    assert_eq!(naming(cx).map(|_| ()), None, "saving closes the field");
    assert_eq!(picked(cx), EqualizerPick::Saved("Late night".into()));
    let listed =
        main_view(cx).read_with(cx, |main, _| main.settings_panel.audio.equalizer.picks.clone());
    assert!(listed.contains(&EqualizerPick::Saved("Late night".into())), "the list shows it");
}

#[gpui::test]
fn cancel_closes_the_name_field(cx: &mut TestAppContext) {
    let cx = open_equalizer(cx);
    open_naming(cx);
    main_view(cx).update(cx, MainWindow::close_naming);
    draw(cx);
    assert_eq!(naming(cx).map(|_| ()), None);
    main_view(cx).update(cx, MainWindow::save_preset);
    assert_eq!(picked(cx), EqualizerPick::None, "nothing to save without the field");
}

#[gpui::test]
fn its_buttons_answer_the_keyboard(cx: &mut TestAppContext) {
    let cx = open_equalizer(cx);
    let presets =
        main_view(cx).read_with(cx, |main, _| main.settings_panel.audio.equalizer.presets.clone());
    let handle = presets.read_with(cx, Focusable::focus_handle);
    press_near(cx, &handle, 1, "space");
    let two_pass = cx.update(|_, cx| effects::value(cx, TWO_PASS));
    assert_eq!(two_pass, Some(Value::Bool(true)), "2 Pass is on");
    press_near(cx, &handle, -1, "space");
    let field = naming(cx).expect("Save as preset… opened the name field");
    let typing = field.read_with(cx, Focusable::focus_handle);
    press_near(cx, &typing, 1, "space");
    assert_eq!(naming(cx).map(|_| ()), None, "Cancel closed it");
    press_near(cx, &handle, -1, "space");
    let field = naming(cx).expect("the name field again");
    submit(cx, &field, "Mine");
    assert_eq!(picked(cx), EqualizerPick::Saved("Mine".into()));
}
