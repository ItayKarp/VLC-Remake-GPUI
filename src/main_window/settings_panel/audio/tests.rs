//! The Audio tab in a window playing the mock's clip: the stereo mode and its Test, the audio
//! track's delay typed or slid, and the notes over the effects.

use std::time::Duration;

use gpui::{Focusable, TestAppContext, VisualTestContext};

use super::*;
use crate::engine::backend::Command;
use crate::main_window::now_playing::tests::{Playing, draw, osd, play_mock, with_tracks};
use crate::main_window::settings_panel::tests::{
    field_text, leave, press_near, press_on, show_tab, submit, type_over,
};
use crate::main_window::tests::{main_view, open};
use crate::settings;
use crate::ui::text_input::TextInput;

/// The Audio tab over the mock's clip with an audio track of `channels`.
fn open_audio(cx: &mut TestAppContext, channels: u32) -> (&mut VisualTestContext, Playing) {
    let cx = open(cx);
    let playing = play_mock(cx, with_tracks(channels));
    show_tab(cx, PanelTab::Audio);
    (cx, playing)
}

/// The delay row's field.
fn delay_field(cx: &mut VisualTestContext) -> Entity<TextInput> {
    let row = main_view(cx).read_with(cx, |main, _| main.settings_panel.audio.delay.clone());
    row.read_with(cx, |row, _| row.field().clone())
}

fn delay(cx: &mut VisualTestContext, playing: &Playing) -> i64 {
    playing.player.read_with(cx, |player, _| player.audio_delay().as_millis())
}

#[test]
fn stereo_mode_offers_the_designed_choices() {
    let labels: Vec<_> = stereo_choices().iter().map(|choice| choice.label).collect();
    assert_eq!(
        labels,
        ["Stereo", "Mono", "Left", "Right", "Reverse stereo", "Dolby Surround", "Headphones"]
    );
}

#[gpui::test]
fn the_stereo_mode_holds_for_the_session(cx: &mut TestAppContext) {
    let (cx, _playing) = open_audio(cx, 2);
    let stereo = main_view(cx).read_with(cx, |main, _| main.settings_panel.audio.stereo.clone());
    press_on(cx, &stereo, "end");
    let last = stereo_choices().last().expect("choices").id;
    assert_eq!(cx.update(|_, cx| effects::value(cx, STEREO_MODE)), Some(Value::Choice(last)));
    assert_eq!(cx.update(|_, cx| stereo_index(cx)), stereo_choices().len() - 1);
}

#[gpui::test]
fn test_plays_the_tone_and_says_which_side_sounds(cx: &mut TestAppContext) {
    let (cx, playing) = open_audio(cx, 2);
    playing.mock.take_commands();
    main_view(cx).update(cx, MainWindow::test_stereo);
    assert!(playing.mock.take_commands().contains(&Command::PlayTestTone));
    assert_eq!(osd(cx), "Test tone: left");
    cx.executor().advance_clock(test_tone::RIGHT_STARTS + Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(osd(cx), "Test tone: right");
}

#[gpui::test]
fn a_typed_delay_moves_the_audio_track(cx: &mut TestAppContext) {
    let (cx, playing) = open_audio(cx, 2);
    let field = delay_field(cx);
    submit(cx, &field, "0.25");
    assert_eq!(delay(cx, &playing), 250);
    assert_eq!(osd(cx), "Audio delay: +250 ms");
    type_over(cx, &field, "-1");
    leave(cx);
    assert_eq!(delay(cx, &playing), -1000, "leaving the field takes it");
    submit(cx, &field, "later");
    assert!(field_text(cx, &field).1, "a reason shows");
    assert_eq!(delay(cx, &playing), -1000);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!field_text(cx, &field).1, "Escape puts the delay in use back");
    assert!(main_view(cx).read_with(cx, |main, _| main.settings_panel.is_open()));
}

#[gpui::test]
fn the_tab_says_what_the_effects_cant_do(cx: &mut TestAppContext) {
    let (cx, _playing) = open_audio(cx, 1);
    let main = main_view(cx);
    assert!(main.read_with(cx, MainWindow::plays_mono));
    let target = main.read_with(cx, MainWindow::audio_target).expect("a track plays");
    assert!(target.starts_with("For “"), "{target}");
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.audio.passthrough = true));
    draw(cx);
    assert!(cx.update(|_, cx| effects::effective(cx).prefs.audio.passthrough));
}

#[gpui::test]
fn without_audio_the_delay_waits_for_a_track(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, crate::engine::mock::MockMedia::default());
    show_tab(cx, PanelTab::Audio);
    let main = main_view(cx);
    assert_eq!(main.read_with(cx, MainWindow::audio_target), None);
    assert!(!main.read_with(cx, MainWindow::plays_mono));
    main.update(cx, |main, cx| main.set_audio_delay(Offset::from_millis(100), cx));
    assert_eq!(delay(cx, &playing), 0);
    assert_eq!(osd(cx), "No audio playing");
}

#[gpui::test]
fn test_answers_the_keyboard(cx: &mut TestAppContext) {
    let (cx, playing) = open_audio(cx, 2);
    let stereo = main_view(cx).read_with(cx, |main, _| main.settings_panel.audio.stereo.clone());
    let handle = stereo.read_with(cx, Focusable::focus_handle);
    playing.mock.take_commands();
    press_near(cx, &handle, -1, "space");
    assert!(playing.mock.take_commands().contains(&Command::PlayTestTone));
}
