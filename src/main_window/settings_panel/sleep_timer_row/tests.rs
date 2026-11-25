//! The Playback tab's Sleep Timer row used from the keyboard: picking an entry starts or ends
//! the timer, Custom opens its Minutes field, and the field starts the timer or says what it
//! takes.

use gpui::{Entity, Focusable, TestAppContext, VisualTestContext};

use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{
    field_text, open_tab, press_near, press_on, submit,
};
use crate::main_window::sleep_timer::rules::Choice;
use crate::main_window::tests::main_view;
use crate::ui::dropdown::Dropdown;
use crate::ui::text_input::TextInput;

fn controls(cx: &mut VisualTestContext) -> (Entity<Dropdown>, Entity<TextInput>) {
    main_view(cx).read_with(cx, |main, _| {
        let control = &main.settings_panel.sleep_timer;
        (control.dropdown.clone(), control.minutes.clone())
    })
}

fn shown(cx: &mut VisualTestContext) -> (Choice, Option<String>) {
    main_view(cx).read_with(cx, |main, cx| (main.sleep_choice(), main.sleep_hint(cx)))
}

#[gpui::test]
fn picking_a_length_starts_the_timer_and_off_ends_it(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Playback);
    let (dropdown, _) = controls(cx);
    press_on(cx, &dropdown, "3");
    assert_eq!(shown(cx), (Choice::Minutes(30), Some("Stops in 30 min".into())));
    assert_eq!(dropdown.read_with(cx, |dropdown, _| dropdown.selected()), 2);
    press_on(cx, &dropdown, "home");
    assert_eq!(shown(cx), (Choice::Off, None));
}

#[gpui::test]
fn custom_takes_typed_minutes(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Playback);
    let (dropdown, minutes) = controls(cx);
    press_on(cx, &dropdown, "end");
    assert_eq!(shown(cx), (Choice::Custom, None));
    let focused = cx.update(|window, cx| minutes.read(cx).is_focused(window));
    assert!(focused, "Custom moves the keyboard to the Minutes field");
    submit(cx, &minutes, "never");
    assert_eq!(field_text(cx, &minutes), ("never".into(), true));
    assert_eq!(shown(cx), (Choice::Custom, None), "nothing starts");
    submit(cx, &minutes, "25");
    assert_eq!(field_text(cx, &minutes), ("25".into(), false));
    assert_eq!(shown(cx), (Choice::Custom, Some("Stops in 25 min".into())));
}

#[gpui::test]
fn shift_tab_from_the_timer_reaches_show_statistics(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Playback);
    let (dropdown, _) = controls(cx);
    let handle = dropdown.read_with(cx, Focusable::focus_handle);
    press_near(cx, &handle, -1, "space");
    assert!(main_view(cx).read_with(cx, |main, _| main.shows_statistics()));
}
