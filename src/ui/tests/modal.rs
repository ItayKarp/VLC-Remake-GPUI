//! Modal dialog: focus trapping, the default button, and nested dialogs.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Context, Entity, FocusHandle, InputEvent, Render, TestAppContext,
    VisualTestContext, Window, div, prelude::*, px,
};

use super::support::{draw, host, setup};
use crate::ui::modal::{Dialog, DialogButton, ModalLayer};

struct ModalHost {
    layer: Entity<ModalLayer>,
    outside: FocusHandle,
    pressed: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for ModalHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(div().id("outside").track_focus(&self.outside).size(px(10.)))
            .child(self.layer.clone())
    }
}

fn open_dialog(
    cx: &mut TestAppContext,
) -> (Entity<ModalHost>, Entity<Dialog>, &mut VisualTestContext) {
    let (host, cx) = host(cx, |_, cx| ModalHost {
        layer: cx.new(|_| ModalLayer::new()),
        outside: cx.focus_handle().tab_stop(true),
        pressed: Rc::default(),
    });
    // Focus starts outside, as it would on the button that opened the dialog.
    host.update_in(cx, |this, window, cx| this.outside.focus(window, cx));
    draw(cx);
    let dialog = host.update_in(cx, |this, window, cx| {
        let pressed = this.pressed.clone();
        let press = move |name: &'static str| {
            let pressed = pressed.clone();
            move |_: &mut Window, _: &mut gpui::App| pressed.borrow_mut().push(name)
        };
        let dialog = cx.new(|cx| {
            Dialog::new(
                "Test",
                |_, _| div().child("Body").into_any_element(),
                vec![
                    DialogButton::new("Cancel").on_click(press("cancel")),
                    DialogButton::new("OK").primary().on_click(press("ok")),
                ],
                cx,
            )
        });
        this.layer.update(cx, |layer, cx| layer.push(dialog.clone(), window, cx));
        dialog
    });
    // One frame draws the dialog, the next moves focus into it.
    draw(cx);
    draw(cx);
    (host, dialog, cx)
}

/// A full key press. `simulate_keystrokes` sends only key down, and GPUI clicks a
/// focused button on key up.
fn press(cx: &mut VisualTestContext, key: &'static str) {
    let keystroke = gpui::Keystroke::parse(key).unwrap();
    cx.update(|window, cx| {
        window.dispatch_event(
            gpui::KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
        window.dispatch_event(gpui::KeyUpEvent { keystroke }.to_platform_input(), cx);
    });
    draw(cx);
}

fn focused_button(dialog: &Entity<Dialog>, cx: &mut VisualTestContext) -> Option<usize> {
    let handles = dialog.read_with(cx, |d, _| d.button_focus_handles());
    cx.update(|window, _| handles.iter().position(|h| h.is_focused(window)))
}

#[gpui::test]
fn dialog_traps_tab_and_restores_focus(cx: &mut TestAppContext) {
    setup(cx);
    let (host, dialog, cx) = open_dialog(cx);
    assert_eq!(focused_button(&dialog, cx), Some(0), "focus moves into the dialog");

    cx.simulate_keystrokes("tab");
    draw(cx);
    assert_eq!(focused_button(&dialog, cx), Some(1));
    cx.simulate_keystrokes("tab");
    draw(cx);
    assert_eq!(focused_button(&dialog, cx), Some(0), "Tab wraps inside the dialog");
    cx.simulate_keystrokes("shift-tab");
    draw(cx);
    assert_eq!(focused_button(&dialog, cx), Some(1), "Shift+Tab wraps backward");

    cx.simulate_keystrokes("escape");
    draw(cx);
    let outside = host.read_with(cx, |h, _| h.outside.clone());
    assert!(cx.update(|window, _| outside.is_focused(window)));
    assert!(host.read_with(cx, |h, cx| !h.layer.read(cx).is_open()));
    assert!(host.read_with(cx, |h, _| h.pressed.borrow().is_empty()));
}

#[gpui::test]
fn enter_presses_the_default_button_unless_another_has_focus(cx: &mut TestAppContext) {
    setup(cx);
    let (host, dialog, cx) = open_dialog(cx);
    // Focus is on Cancel: Enter clicks Cancel, not the default.
    assert_eq!(focused_button(&dialog, cx), Some(0));
    press(cx, "enter");
    draw(cx);
    assert_eq!(host.read_with(cx, |h, _| h.pressed.borrow().clone()), ["cancel"]);
}

#[gpui::test]
fn nested_dialogs_close_one_at_a_time(cx: &mut TestAppContext) {
    setup(cx);
    let (host, _, cx) = open_dialog(cx);
    host.update_in(cx, |this, window, cx| {
        let second = cx.new(|cx| {
            Dialog::new(
                "Second",
                |_, _| div().into_any_element(),
                vec![DialogButton::new("OK").primary()],
                cx,
            )
        });
        this.layer.update(cx, |layer, cx| layer.push(second, window, cx));
    });
    draw(cx);
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(host.read_with(cx, |h, cx| h.layer.read(cx).is_open()));
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(host.read_with(cx, |h, cx| !h.layer.read(cx).is_open()));
}
