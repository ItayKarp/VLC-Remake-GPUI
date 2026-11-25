//! The Subtitles tab's Synchronization: disabled with no subtitle showing, and the delay, speed
//! and duration rows changing the one showing.

use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};

use super::SyncValue;
use crate::main_window::now_playing::tests::{Playing, draw, play_mock, with_tracks};
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{open_tab, press_near, show_tab, submit};
use crate::main_window::tests::{main_view, open};
use crate::ui::text_input::TextInput;

fn delay_field(cx: &mut VisualTestContext) -> Entity<TextInput> {
    let row = main_view(cx).read_with(cx, |main, _| main.settings_panel.subtitles.delay.clone());
    row.read_with(cx, |row, _| row.field().clone())
}

fn delay(cx: &mut VisualTestContext, playing: &Playing) -> i64 {
    playing.player.read_with(cx, |player, _| player.subtitle_sync().delay().as_millis())
}

#[gpui::test]
fn the_delay_moves_the_subtitle_showing(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, with_tracks(2));
    show_tab(cx, PanelTab::Subtitles);
    let field = delay_field(cx);
    submit(cx, &field, "1.5");
    assert_eq!(delay(cx, &playing), 1500);
    let main = main_view(cx);
    let showing = main.update(cx, |main, cx| main.showing_sync(cx)).expect("a subtitle shows");
    assert!(showing.target.starts_with("For “"), "{}", showing.target);
    assert!(!showing.sync.is_default());
    main.update(cx, |main, cx| main.change_sync(SyncValue::Speed, 1.1, cx));
    main.update(cx, |main, cx| main.change_sync(SyncValue::Duration, 2., cx));
    draw(cx);
    assert_eq!(delay(cx, &playing), 1500, "the other values keep the delay");
}

#[gpui::test]
fn without_a_subtitle_nothing_syncs(cx: &mut TestAppContext) {
    let (cx, playing) = open_tab(cx, PanelTab::Subtitles);
    let main = main_view(cx);
    assert!(main.update(cx, |main, cx| main.showing_sync(cx)).is_none());
    main.update(cx, |main, cx| main.change_sync(SyncValue::Delay, 2., cx));
    draw(cx);
    assert_eq!(delay(cx, &playing), 0);
}

#[gpui::test]
fn reset_answers_the_keyboard(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, with_tracks(2));
    show_tab(cx, PanelTab::Subtitles);
    let field = delay_field(cx);
    submit(cx, &field, "2");
    assert_eq!(delay(cx, &playing), 2000);
    let duration =
        main_view(cx).read_with(cx, |main, _| main.settings_panel.subtitles.duration.clone());
    let slider = duration.read_with(cx, |row, _| row.slider().clone());
    let handle = slider.read_with(cx, Focusable::focus_handle);
    press_near(cx, &handle, 1, "space");
    assert_eq!(delay(cx, &playing), 0, "Reset put the sync back");
}
