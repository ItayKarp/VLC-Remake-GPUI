//! Window tests of the Keyboard Shortcuts dialog's rows as keys are recorded: the row being
//! recorded, a changed row's reset button, and the notes under a row for a refused key or one
//! another command has.

use gpui::{
    AppContext as _, Context, Entity, IntoElement, Render, TestAppContext, VisualTestContext,
    Window, div, prelude::*,
};

use super::editor::{Note, ShortcutsEditor};
use crate::keymap::actions::Stop;
use crate::keymap::custom::Shortcuts;
use crate::main_window::device_dialog::tests::{draw, open_view};
use crate::ui::modal::Dialog;

/// The dialog, drawn as the window's content, and the edits it shows.
struct Host {
    editor: Entity<ShortcutsEditor>,
    dialog: Entity<Dialog>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.dialog.clone())
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<ShortcutsEditor>, &mut VisualTestContext) {
    let (host, cx) = open_view(cx, |window, cx| {
        let os = gpui::App::platform_info(cx).os;
        let editor = cx.new(|_| ShortcutsEditor::new(Shortcuts::defaults(os)));
        let dialog = super::build(editor.clone(), window, cx);
        Host { editor, dialog }
    });
    let editor = host.read_with(cx, |host, _| host.editor.clone());
    (editor, cx)
}

fn stop() -> usize {
    Shortcuts::index_of(&Stop).expect("Stop is a command")
}

/// Starts recording Stop's key and draws the row recording.
fn record(editor: &Entity<ShortcutsEditor>, cx: &mut VisualTestContext) {
    editor.update_in(cx, |editor, window, cx| editor.record(stop(), window, cx));
    draw(cx);
    assert_eq!(editor.read_with(cx, |editor, _| editor.recording()), Some(stop()));
}

/// Presses `key` and draws what follows.
fn press(key: &str, cx: &mut VisualTestContext) {
    cx.simulate_keystrokes(key);
    draw(cx);
}

fn note(editor: &Entity<ShortcutsEditor>, cx: &mut VisualTestContext) -> Option<Note> {
    editor.read_with(cx, |editor, _| editor.note(stop()).cloned())
}

#[gpui::test]
fn a_changed_row_can_be_reset(cx: &mut TestAppContext) {
    let (editor, cx) = open(cx);
    record(&editor, cx);
    press("z", cx);
    assert!(editor.read_with(cx, |editor, _| editor.is_dirty()), "the row shows its reset");
    editor.update(cx, |editor, cx| editor.reset(stop(), cx));
    draw(cx);
    assert!(!editor.read_with(cx, |editor, _| editor.is_dirty()));
}

#[gpui::test]
fn a_taken_key_offers_replace_or_cancel(cx: &mut TestAppContext) {
    let (editor, cx) = open(cx);
    record(&editor, cx);
    press("space", cx);
    assert!(matches!(note(&editor, cx), Some(Note::Taken(..))));
    editor.update(cx, ShortcutsEditor::dismiss_note);
    draw(cx);
    assert_eq!(note(&editor, cx), None);
}

#[gpui::test]
fn a_system_key_is_refused_with_why(cx: &mut TestAppContext) {
    let (editor, cx) = open(cx);
    record(&editor, cx);
    press(if cfg!(target_os = "macos") { "cmd-m" } else { "alt-f4" }, cx);
    assert!(matches!(note(&editor, cx), Some(Note::Refused(_))));
}
