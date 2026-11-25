//! The Playback tab's speed controls: typed speeds change the player's, text that isn't one
//! turns the field red, Escape and leaving put the speed playing back, and live streams disable
//! both. Show Statistics shows the overlay.

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};

use crate::engine::backend::Command;
use crate::engine::mock::MockMedia;
use crate::main_window::now_playing::tests::{Playing, draw, play_mock};
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{
    field_text, leave, open_tab, show_tab, submit, type_over,
};
use crate::main_window::tests::{main_view, open};
use crate::ui::slider::Slider;
use crate::ui::text_input::TextInput;

fn controls(cx: &mut VisualTestContext) -> (Entity<TextInput>, Entity<Slider>) {
    main_view(cx).read_with(cx, |main, _| {
        let speed = &main.settings_panel.speed;
        (speed.field.clone(), speed.slider.clone())
    })
}

fn rate(cx: &mut VisualTestContext, playing: &Playing) -> f32 {
    playing.player.read_with(cx, |player, _| player.rate().get())
}

#[gpui::test]
fn a_typed_speed_plays_and_shows(cx: &mut TestAppContext) {
    let (cx, playing) = open_tab(cx, PanelTab::Playback);
    let (field, slider) = controls(cx);
    submit(cx, &field, "150%");
    assert!((rate(cx, &playing) - 1.5).abs() < 1e-6);
    assert_eq!(field_text(cx, &field), ("1.5x".into(), false));
    assert!((slider.read_with(cx, |slider, _| slider.value()) - 1.5).abs() < 1e-6);
    submit(cx, &field, "3");
    assert!((slider.read_with(cx, |slider, _| slider.value()) - 2.).abs() < 1e-6, "at its end");
}

#[gpui::test]
fn text_that_isnt_a_speed_changes_nothing_and_escape_puts_it_back(cx: &mut TestAppContext) {
    let (cx, playing) = open_tab(cx, PanelTab::Playback);
    let (field, _) = controls(cx);
    submit(cx, &field, "quick");
    assert_eq!(field_text(cx, &field), ("quick".into(), true));
    assert!((rate(cx, &playing) - 1.).abs() < 1e-6);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert_eq!(field_text(cx, &field), ("1x".into(), false));
    assert!(main_view(cx).read_with(cx, |main, _| main.settings_panel.is_open()));
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(!main_view(cx).read_with(cx, |main, _| main.settings_panel.is_open()));
}

#[gpui::test]
fn leaving_the_field_takes_a_speed_typed(cx: &mut TestAppContext) {
    let (cx, playing) = open_tab(cx, PanelTab::Playback);
    let (field, _) = controls(cx);
    type_over(cx, &field, "0.5");
    leave(cx);
    assert!((rate(cx, &playing) - 0.5).abs() < 1e-6);
    assert_eq!(field_text(cx, &field), ("0.5x".into(), false));
    type_over(cx, &field, "slow");
    leave(cx);
    assert_eq!(field_text(cx, &field), ("0.5x".into(), false), "the speed playing shows");
}

#[gpui::test]
fn a_live_stream_disables_the_speed(cx: &mut TestAppContext) {
    let cx = open(cx);
    let live = MockMedia { duration: None, seekable: false, ..MockMedia::default() };
    let playing = play_mock(cx, live);
    show_tab(cx, PanelTab::Playback);
    let (field, _) = controls(cx);
    submit(cx, &field, "2");
    assert!((rate(cx, &playing) - 1.).abs() < 1e-6, "a live stream keeps its speed");
}

#[gpui::test]
fn show_statistics_reads_them_every_second_while_on(cx: &mut TestAppContext) {
    let (cx, playing) = open_tab(cx, PanelTab::Playback);
    playing.mock.take_commands();
    let main = main_view(cx);
    main.update(cx, |main, cx| main.set_show_statistics(true, cx));
    draw(cx);
    cx.executor().advance_clock(Duration::from_secs(1));
    draw(cx);
    let reads = |playing: &Playing| {
        let commands = playing.mock.take_commands();
        commands.iter().filter(|c| **c == Command::ReadStatistics).count()
    };
    assert_eq!(reads(&playing), 2);
    assert!(main.read_with(cx, |main, _| main.shows_statistics()));
    main.update(cx, |main, cx| main.set_show_statistics(false, cx));
    cx.executor().advance_clock(Duration::from_secs(3));
    draw(cx);
    assert_eq!(reads(&playing), 0);
}
