//! Headless window tests for the kit's keyboard and focus behavior, on GPUI's
//! test platform (no real window or input).

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Context, DismissEvent, Entity, FocusHandle, Focusable, InputEvent, Render,
    SharedString, Subscription, TestAppContext, VisualTestContext, Window, div, prelude::*, px,
};

use super::dropdown::{self, Dropdown};
use super::menu::{ContextMenu, MenuItem};
use super::modal::{Dialog, DialogButton, ModalLayer};
use super::slider::{self, Slider, SliderModel};
use super::text_input::TextInput;

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        super::init(cx);
    });
}

/// Paints a frame (so focus, tab stops and bounds exist) and settles tasks.
fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

/// A window that shows one child view, with Tab handling like the app's windows.
struct Host {
    child: gpui::AnyView,
    focus: FocusHandle,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::focus::tab_navigation(
            div().track_focus(&self.focus).size_full().child(self.child.clone()),
        )
    }
}

fn host<V: Render>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
) -> (Entity<V>, &mut VisualTestContext) {
    let slot: Rc<RefCell<Option<Entity<V>>>> = Rc::default();
    let out = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let child = cx.new(|cx| build(window, cx));
        *out.borrow_mut() = Some(child.clone());
        Host { child: child.into(), focus: cx.focus_handle() }
    });
    let child = slot.borrow_mut().take().unwrap();
    draw(cx);
    (child, cx)
}

// ---------------------------------------------------------------------------
// Context menu
// ---------------------------------------------------------------------------

struct MenuHost {
    menu: Option<Entity<ContextMenu>>,
    chosen: Rc<RefCell<Vec<&'static str>>>,
    _subscription: Option<Subscription>,
}

impl Render for MenuHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().children(
            self.menu
                .clone()
                .map(|menu| super::popover::at_position(gpui::point(px(10.), px(10.)), 1, menu)),
        )
    }
}

fn open_menu(cx: &mut TestAppContext) -> (Entity<MenuHost>, &mut VisualTestContext) {
    let (host, cx) =
        host(cx, |_, _| MenuHost { menu: None, chosen: Rc::default(), _subscription: None });
    host.update_in(cx, |this, window, cx| {
        let pick = |name: &'static str, chosen: &Rc<RefCell<Vec<&'static str>>>| {
            let chosen = chosen.clone();
            move |_: &mut Window, _: &mut gpui::App| chosen.borrow_mut().push(name)
        };
        let items = vec![
            MenuItem::action("Play", pick("play", &this.chosen)),
            MenuItem::Separator,
            MenuItem::action("Convert", pick("convert", &this.chosen)).disabled(true),
            MenuItem::submenu(
                "Sort By",
                vec![
                    MenuItem::radio("Name", true, pick("name", &this.chosen)),
                    MenuItem::radio("Duration", false, pick("duration", &this.chosen)),
                ],
            ),
        ];
        let menu = ContextMenu::build(items, window, cx);
        this._subscription = Some(cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.menu = None;
            cx.notify();
        }));
        this.menu = Some(menu);
        cx.notify();
    });
    draw(cx);
    (host, cx)
}

#[gpui::test]
fn menu_skips_separators_and_disabled_items_and_activates(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = open_menu(cx);
    // Down: Play. Down: skips the separator and disabled Convert to Sort By.
    // Down: wraps to Play.
    cx.simulate_keystrokes("down down down enter");
    draw(cx);
    let host = host.read_with(cx, |h, _| (h.menu.is_none(), h.chosen.borrow().clone()));
    assert_eq!(host, (true, vec!["play"]));
}

#[gpui::test]
fn escape_in_a_submenu_closes_only_the_submenu(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = open_menu(cx);
    cx.simulate_keystrokes("up right");
    draw(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(
        host.read_with(cx, |h, _| h.menu.is_some()),
        "Escape in the submenu closed the whole menu"
    );
    // Back in the parent: open the submenu again and choose its second item.
    cx.simulate_keystrokes("right");
    draw(cx);
    cx.simulate_keystrokes("down enter");
    draw(cx);
    let chosen = host.read_with(cx, |h, _| (h.menu.is_none(), h.chosen.borrow().clone()));
    assert_eq!(chosen, (true, vec!["duration"]));
}

#[gpui::test]
fn escape_at_the_top_closes_the_menu(cx: &mut TestAppContext) {
    setup(cx);
    let (host, cx) = open_menu(cx);
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(host.read_with(cx, |h, _| h.menu.is_none()));
}

// ---------------------------------------------------------------------------
// Modal dialog
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Dropdown, slider, text input
// ---------------------------------------------------------------------------

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
            sink.borrow_mut().push(event.0)
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
    struct Two {
        a: Entity<TextInput>,
        b: Entity<TextInput>,
    }
    impl Render for Two {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child(self.a.clone()).child(self.b.clone())
        }
    }
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
