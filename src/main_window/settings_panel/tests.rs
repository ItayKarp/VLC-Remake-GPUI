//! The settings panel in a window playing the mock's clip: each tab drawn, and its controls used
//! as a person does, by typing, keys and the values they change. The helpers here serve the
//! tests of each tab's file.

mod leaving_fields;

use gpui::{Entity, FocusHandle, Focusable as _, TestAppContext, VisualTestContext};

use super::PanelTab;
use crate::effects::EffectSection;
use crate::main_window::now_playing::tests::{Playing, draw, open_playing};
use crate::main_window::tests::{main_view, tap};
use crate::settings;
use crate::ui::dropdown::Dropdown;
use crate::ui::text_input::TextInput;
use crate::ui::text_input::keys::Submit;

/// The window playing the mock's clip, the panel open on `tab` and drawn.
pub(super) fn open_tab(
    cx: &mut TestAppContext,
    tab: PanelTab,
) -> (&mut VisualTestContext, Playing) {
    let (cx, playing) = open_playing(cx);
    show_tab(cx, tab);
    (cx, playing)
}

/// Opens the panel on `tab` and draws it.
pub(super) fn show_tab(cx: &mut VisualTestContext, tab: PanelTab) {
    main_view(cx).update_in(cx, |main, window, cx| main.open_settings_panel(tab, window, cx));
    draw(cx);
}

/// Opens `sections`, as clicking their Expand Settings does, and draws them.
pub(super) fn expand(cx: &mut VisualTestContext, sections: &[EffectSection]) {
    let ids: Vec<String> = sections.iter().map(|section| section.id.to_owned()).collect();
    cx.update(|_, cx| settings::update(cx, |s| s.local.expanded_effects = ids));
    draw(cx);
}

/// Types `text` over what `field` holds and presses Enter. Enter is sent as the action it is
/// bound to: the test platform would also type it into the field as text.
pub(super) fn submit(cx: &mut VisualTestContext, field: &Entity<TextInput>, text: &str) {
    type_over(cx, field, text);
    cx.dispatch_action(Submit);
    draw(cx);
}

/// Focuses `field` and types `text` over what it holds. The window is made active first, as
/// a window being typed into is, so that leaving the field later counts.
pub(super) fn type_over(cx: &mut VisualTestContext, field: &Entity<TextInput>, text: &str) {
    cx.update(|window, cx| {
        window.activate_window();
        field.read(cx).focus_handle(cx).focus(window, cx);
        field.update(cx, TextInput::select_everything);
    });
    draw(cx);
    cx.simulate_input(text);
    draw(cx);
}

/// Moves the keyboard to the player, leaving the focused field.
pub(super) fn leave(cx: &mut VisualTestContext) {
    let main = main_view(cx);
    cx.update(|window, cx| main.read(cx).focus_handle.clone().focus(window, cx));
    draw(cx);
}

/// Focuses `dropdown` and presses `keys` on it, closed, as picking by keyboard does.
pub(super) fn press_on(cx: &mut VisualTestContext, dropdown: &Entity<Dropdown>, keys: &str) {
    cx.update(|window, cx| dropdown.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);
    cx.simulate_keystrokes(keys);
    draw(cx);
}

/// Focuses what `handle` belongs to, moves the keyboard `steps` tab stops on (back for a
/// negative count), as Tab and Shift-Tab do, and presses and releases `key` there.
pub(super) fn press_near(
    cx: &mut VisualTestContext,
    handle: &FocusHandle,
    steps: i32,
    key: &'static str,
) {
    cx.update(|window, cx| handle.focus(window, cx));
    draw(cx);
    for _ in 0..steps.unsigned_abs() {
        cx.update(
            |window, cx| {
                if steps < 0 { window.focus_prev(cx) } else { window.focus_next(cx) }
            },
        );
        draw(cx);
    }
    tap(cx, key);
    draw(cx);
}

/// The text `field` holds, and whether it shows an error.
pub(super) fn field_text(cx: &mut VisualTestContext, field: &Entity<TextInput>) -> (String, bool) {
    field.read_with(cx, |field, _| (field.text().to_owned(), field.is_invalid()))
}

fn open_tab_of(cx: &mut VisualTestContext) -> Option<PanelTab> {
    main_view(cx).read_with(cx, |main, _| main.settings_panel.open)
}

#[gpui::test]
fn every_tab_draws_over_playing_media(cx: &mut TestAppContext) {
    let (cx, _playing) = open_tab(cx, PanelTab::Playback);
    for tab in PanelTab::ALL {
        show_tab(cx, tab);
        assert_eq!(open_tab_of(cx), Some(tab));
    }
}

#[gpui::test]
fn the_panel_closes_once_its_media_is_gone(cx: &mut TestAppContext) {
    let (cx, playing) = open_tab(cx, PanelTab::Video);
    playing.player.update(cx, crate::engine::player::Player::stop);
    draw(cx);
    assert_eq!(open_tab_of(cx), None);
}
