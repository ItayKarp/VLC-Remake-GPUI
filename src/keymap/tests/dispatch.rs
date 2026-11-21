//! Which keys reach the player (S0.5.2): headless windows with the real keymap,
//! focus on the player, a text input, a button or inside a modal.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*,
};

use crate::keymap::actions as act;
use crate::keymap::contexts::{MAIN_WINDOW, PLAYER};
use crate::keymap::{self, label::KeyCap};
use crate::ui::button::Button;
use crate::ui::focus;
use crate::ui::modal;
use crate::ui::text_input::TextInput;

const OPEN: &str = if cfg!(target_os = "macos") { "cmd-o" } else { "ctrl-o" };

/// No action ran.
const NONE: [&str; 0] = [];

type Seen = Rc<RefCell<Vec<&'static str>>>;

/// A stand-in for the main window: the player with a button, a text input and an
/// open modal inside it.
struct Player {
    root: FocusHandle,
    button: FocusHandle,
    modal: FocusHandle,
    input: Entity<TextInput>,
    seen: Seen,
}

fn record<A: gpui::Action>(
    seen: &Seen,
    name: &'static str,
) -> impl Fn(&A, &mut Window, &mut gpui::App) + use<A> {
    let seen = seen.clone();
    move |_, _, _| seen.borrow_mut().push(name)
}

impl Render for Player {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context(focus::context(&[MAIN_WINDOW, PLAYER]))
            .track_focus(&self.root)
            .on_action(record::<act::PlayPause>(&self.seen, "play"))
            .on_action(record::<act::ToggleFullscreen>(&self.seen, "fullscreen"))
            .on_action(record::<act::OpenFile>(&self.seen, "open"))
            .child(Button::new("button", "Button").track_focus(&self.button))
            .child(self.input.clone())
            .child(div().key_context(modal::CONTEXT).track_focus(&self.modal).child("Dialog"))
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Player>, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
        keymap::init(cx);
    });
    let (player, cx) = cx.add_window_view(|window, cx| Player {
        root: cx.focus_handle(),
        button: cx.focus_handle(),
        modal: cx.focus_handle(),
        input: cx.new(|cx| TextInput::new(window, cx)),
        seen: Rc::default(),
    });
    cx.run_until_parked();
    (player, cx)
}

/// Focuses what `pick` returns, types `keys`, and returns the actions that ran.
fn press(
    player: &Entity<Player>,
    cx: &mut VisualTestContext,
    pick: impl Fn(&Player, &gpui::App) -> FocusHandle,
    keys: &str,
) -> Vec<&'static str> {
    player.update_in(cx, |this, window, cx| {
        this.seen.borrow_mut().clear();
        pick(this, cx).focus(window, cx);
    });
    cx.run_until_parked();
    cx.simulate_keystrokes(keys);
    player.read_with(cx, |this, _| this.seen.borrow().clone())
}

#[gpui::test]
fn the_player_gets_its_keys_when_it_has_focus(cx: &mut TestAppContext) {
    let (player, cx) = open(cx);
    let root = |p: &Player, _: &gpui::App| p.root.clone();
    assert_eq!(press(&player, cx, root, "f"), ["fullscreen"]);
    assert_eq!(press(&player, cx, root, "space"), ["play"]);
    assert_eq!(press(&player, cx, root, OPEN), ["open"]);
}

#[gpui::test]
fn typing_in_a_text_input_triggers_no_player_shortcut(cx: &mut TestAppContext) {
    let (player, cx) = open(cx);
    let input = |p: &Player, cx: &gpui::App| p.input.read(cx).focus_handle(cx);
    assert_eq!(press(&player, cx, input, "f"), NONE);
    assert_eq!(press(&player, cx, input, "space"), NONE);
    // Shortcuts with Ctrl/Cmd still work while typing.
    assert_eq!(press(&player, cx, input, OPEN), ["open"]);
}

#[gpui::test]
fn a_focused_button_keeps_space_but_not_letters(cx: &mut TestAppContext) {
    let (player, cx) = open(cx);
    let button = |p: &Player, _: &gpui::App| p.button.clone();
    assert_eq!(press(&player, cx, button, "space"), NONE);
    assert_eq!(press(&player, cx, button, "f"), ["fullscreen"]);
}

#[gpui::test]
fn a_modal_blocks_player_and_app_shortcuts(cx: &mut TestAppContext) {
    let (player, cx) = open(cx);
    let dialog = |p: &Player, _: &gpui::App| p.modal.clone();
    assert_eq!(press(&player, cx, dialog, "f"), NONE);
    assert_eq!(press(&player, cx, dialog, "space"), NONE);
    assert_eq!(press(&player, cx, dialog, OPEN), NONE);
}

#[gpui::test]
fn labels_come_from_the_keymap(cx: &mut TestAppContext) {
    let (_, cx) = open(cx);
    cx.update(|_, cx| {
        let open = if cfg!(target_os = "macos") { "⌘O" } else { "Ctrl + O" };
        assert_eq!(keymap::shortcut(&act::OpenFile, cx).as_deref(), Some(open));
        // The primary key, not the `<` alias, is the one shown.
        assert_eq!(keymap::shortcut(&act::PreviousItem, cx).as_deref(), Some("P"));
        assert_eq!(
            keymap::key_caps(&act::Snapshot, cx),
            vec![KeyCap::Text("SHIFT".into()), KeyCap::Text("S".into())]
        );
        assert_eq!(keymap::shortcut(&act::CustomizePlayer, cx), None);
    });
}
