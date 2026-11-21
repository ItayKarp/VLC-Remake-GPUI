//! Dropdown, slider and text input: keyboard use, and Tab between controls.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Context, Entity, Focusable, Render, SharedString, TestAppContext, Window, div,
    prelude::*,
};

use super::support::{draw, host, setup};
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::text_input::TextInput;

/// Two text inputs side by side, to move focus between.
struct Two {
    a: Entity<TextInput>,
    b: Entity<TextInput>,
}

impl Render for Two {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(self.a.clone()).child(self.b.clone())
    }
}

#[gpui::test]
fn dropdown_opens_moves_and_chooses_with_the_keyboard(cx: &mut TestAppContext) {
    setup(cx);
    let options: Vec<SharedString> =
        ["Mono", "Stereo", "Left", "Right"].into_iter().map(Into::into).collect();
    let (dropdown, cx) = host(cx, move |window, cx| Dropdown::new("Mode", options, 0, window, cx));
    let changes: Rc<RefCell<Vec<usize>>> = Rc::default();
    let sink = changes.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&dropdown, move |_, event: &dropdown::Changed, _| {
            sink.borrow_mut().push(event.0);
        })
    });
    cx.update(|window, cx| dropdown.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);

    cx.simulate_keystrokes("down down down enter");
    draw(cx);
    assert_eq!(dropdown.read_with(cx, |d, _| d.selected()), 2);
    // Type-to-select while closed: "r" jumps to Right.
    cx.simulate_keystrokes("r");
    draw(cx);
    assert_eq!(dropdown.read_with(cx, |d, _| d.selected()), 3);
    assert_eq!(*changes.borrow(), [2, 3]);
    // Escape closes without choosing.
    cx.simulate_keystrokes("enter down escape");
    draw(cx);
    assert_eq!(dropdown.read_with(cx, |d, _| d.selected()), 3);
}

#[gpui::test]
fn slider_steps_with_arrows_and_clamps(cx: &mut TestAppContext) {
    setup(cx);
    let (slider, cx) = host(cx, |window, cx| {
        Slider::new("Volume", SliderModel::new(0., 100., 1., 50.), window, cx)
    });
    let values: Rc<RefCell<Vec<f64>>> = Rc::default();
    let sink = values.clone();
    let _subscription = cx.update(|_, cx| {
        cx.subscribe(&slider, move |_, event: &slider::Changed, _| sink.borrow_mut().push(event.0))
    });
    cx.update(|window, cx| slider.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);
    cx.simulate_keystrokes("right");
    cx.simulate_keystrokes("shift-right");
    cx.simulate_keystrokes("end");
    cx.simulate_keystrokes("right");
    draw(cx);
    assert_eq!(slider.read_with(cx, |s, _| s.value()), 100.);
    // The last Right at the maximum changes nothing and sends nothing.
    assert_eq!(*values.borrow(), [51., 61., 100.]);
}

#[gpui::test]
fn text_input_types_and_undoes(cx: &mut TestAppContext) {
    setup(cx);
    let (input, cx) = host(cx, TextInput::new);
    cx.update(|window, cx| input.read(cx).focus_handle(cx).focus(window, cx));
    draw(cx);
    cx.simulate_input("hello");
    draw(cx);
    assert_eq!(input.read_with(cx, |i, _| i.text().to_owned()), "hello");
    let undo = if cfg!(target_os = "macos") { "cmd-z" } else { "ctrl-z" };
    cx.simulate_keystrokes(undo);
    draw(cx);
    assert_eq!(input.read_with(cx, |i, _| i.text().to_owned()), "");
}

#[gpui::test]
fn tab_moves_between_controls(cx: &mut TestAppContext) {
    setup(cx);
    let (two, cx) = host(cx, |window, cx| Two {
        a: cx.new(|cx| TextInput::new(window, cx)),
        b: cx.new(|cx| TextInput::new(window, cx)),
    });
    let (a, b) =
        two.read_with(cx, |t, cx| (t.a.read(cx).focus_handle(cx), t.b.read(cx).focus_handle(cx)));
    cx.update(|window, cx| a.focus(window, cx));
    draw(cx);
    cx.simulate_keystrokes("tab");
    draw(cx);
    assert!(cx.update(|window, _| b.is_focused(window)));
    cx.simulate_keystrokes("shift-tab");
    draw(cx);
    assert!(cx.update(|window, _| a.is_focused(window)));
}
