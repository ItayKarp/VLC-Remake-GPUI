//! The Keyboard Shortcuts dialog's edits (S5.6.3–S5.6.6): a draft of every command's keys,
//! the row being recorded, and the note under it (a refused key, or a key another command has).
//! Nothing reaches the keymap until Save.

use gpui::{AnyWindowHandle, Context, Keystroke, Subscription, Window};

use crate::keymap::custom::{Conflict, Shortcuts};

/// What a key pressed while recording does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Capture {
    /// Escape: stop recording and keep the keys.
    Cancel,
    /// Backspace or Delete: the command has no keys.
    Clear,
    /// Still waiting: only a modifier is down.
    Wait,
    /// The key can't be a shortcut, for this reason.
    Refused(String),
    /// Another command has the key; Replace or Cancel decides.
    Taken(Keystroke, Conflict),
    /// The command's key from now on.
    Set(Keystroke),
}

/// What `keystroke` does when recording command `index` against `draft`.
pub fn capture(draft: &Shortcuts, index: usize, keystroke: &Keystroke) -> Capture {
    let plain = !keystroke.modifiers.modified();
    match keystroke.key.as_str() {
        "escape" if plain => return Capture::Cancel,
        "backspace" | "delete" if plain => return Capture::Clear,
        _ if keystroke.is_modifier_key() => return Capture::Wait,
        _ => {}
    }
    let key = Keystroke { key_char: None, ..keystroke.clone() };
    if let Err(refusal) = draft.may_use(index, &key) {
        return Capture::Refused(refusal.message());
    }
    match draft.conflict(index, &key) {
        Some(conflict) => Capture::Taken(key, conflict),
        None => Capture::Set(key),
    }
}

/// The line under a row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Note {
    /// The key pressed can't be a shortcut.
    Refused(String),
    /// The key pressed belongs to another command.
    Taken(Keystroke, Conflict),
}

/// The dialog's edits.
pub struct ShortcutsEditor {
    saved: Shortcuts,
    draft: Shortcuts,
    /// The command whose keys are being recorded, and the hook that takes its keys.
    recording: Option<(usize, Subscription)>,
    /// The row with a note under it, and the note.
    note: Option<(usize, Note)>,
}

impl ShortcutsEditor {
    /// An editor starting from `saved`, the keys the keymap has now.
    pub fn new(saved: Shortcuts) -> Self {
        Self { draft: saved.clone(), saved, recording: None, note: None }
    }

    /// The keys as edited so far.
    pub fn draft(&self) -> &Shortcuts {
        &self.draft
    }

    /// Whether anything differs from the keys the dialog opened with.
    pub fn is_dirty(&self) -> bool {
        self.draft != self.saved
    }

    /// The command being recorded.
    pub fn recording(&self) -> Option<usize> {
        self.recording.as_ref().map(|(index, _)| *index)
    }

    /// The note under command `index`'s row.
    pub fn note(&self, index: usize) -> Option<&Note> {
        self.note.as_ref().filter(|(row, _)| *row == index).map(|(_, note)| note)
    }

    /// Starts recording command `index`'s key: the next key pressed in this window, whatever
    /// it is bound to, goes to [`Self::press`] instead.
    pub fn record(&mut self, index: usize, window: &Window, cx: &mut Context<Self>) {
        let editor = cx.entity().downgrade();
        let this_window: AnyWindowHandle = window.window_handle();
        let hook = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle() != this_window {
                return;
            }
            let taken = editor.update(cx, |editor, cx| editor.press(&event.keystroke, cx));
            if taken.unwrap_or(false) {
                cx.stop_propagation();
            }
        });
        self.recording = Some((index, hook));
        self.note = None;
        cx.notify();
    }

    /// Handles a key pressed while recording; whether it was taken.
    fn press(&mut self, keystroke: &Keystroke, cx: &mut Context<Self>) -> bool {
        let Some(index) = self.recording() else {
            return false;
        };
        match capture(&self.draft, index, keystroke) {
            Capture::Wait => return true,
            Capture::Cancel => {}
            Capture::Clear => self.draft.set(index, Vec::new()),
            Capture::Refused(message) => self.note = Some((index, Note::Refused(message))),
            Capture::Taken(key, conflict) => self.note = Some((index, Note::Taken(key, conflict))),
            Capture::Set(key) => self.draft.set(index, vec![key]),
        }
        self.recording = None;
        cx.notify();
        true
    }

    /// Replace: gives command `index` the key its note offers, taking it from the other command.
    pub fn replace(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(Note::Taken(key, _)) = self.note(index).cloned() {
            self.draft.take_from_others(index, &key);
            self.draft.set(index, vec![key]);
        }
        self.dismiss_note(cx);
    }

    /// Closes the note under a row, keeping the keys as they were.
    pub fn dismiss_note(&mut self, cx: &mut Context<Self>) {
        self.note = None;
        cx.notify();
    }

    /// Gives command `index` its default keys again.
    pub fn reset(&mut self, index: usize, cx: &mut Context<Self>) {
        self.draft.reset(index);
        self.note = None;
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use gpui::OperatingSystem;

    use super::*;
    use crate::keymap::actions as act;

    const OS: OperatingSystem = OperatingSystem::Windows;

    fn key(source: &str) -> Keystroke {
        Keystroke::parse_with_os(source, OS).expect("test keys parse")
    }

    fn stop() -> usize {
        Shortcuts::index_of(&act::Stop).expect("a command")
    }

    #[test]
    fn escape_cancels_and_backspace_clears() {
        let draft = Shortcuts::defaults(OS);
        assert_eq!(capture(&draft, stop(), &key("escape")), Capture::Cancel);
        assert_eq!(capture(&draft, stop(), &key("delete")), Capture::Clear);
        assert_eq!(capture(&draft, stop(), &key("backspace")), Capture::Clear);
        assert_eq!(
            capture(&draft, stop(), &key("ctrl-backspace")),
            Capture::Set(key("ctrl-backspace"))
        );
    }

    #[test]
    fn a_free_key_is_set_and_a_taken_one_names_its_command() {
        let draft = Shortcuts::defaults(OS);
        assert_eq!(capture(&draft, stop(), &key("z")), Capture::Set(key("z")));
        let Capture::Taken(_, conflict) = capture(&draft, stop(), &key("space")) else {
            panic!("Space is Play / pause's");
        };
        assert_eq!(conflict.label(), "Play / pause");
    }

    #[test]
    fn system_keys_and_lone_modifiers_are_not_taken() {
        let draft = Shortcuts::defaults(OS);
        assert!(matches!(capture(&draft, stop(), &key("alt-f4")), Capture::Refused(_)));
        assert_eq!(capture(&draft, stop(), &key("shift")), Capture::Wait);
    }

    /// Holds the editor in a window, as the dialog does.
    struct Host(gpui::Entity<ShortcutsEditor>);

    impl gpui::Render for Host {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    #[gpui::test]
    fn recording_takes_the_next_key_whatever_it_is_bound_to(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;
        let (host, cx) = cx.add_window_view(|_, cx| {
            let os = gpui::App::platform_info(cx).os;
            Host(cx.new(|_| ShortcutsEditor::new(Shortcuts::defaults(os))))
        });
        let editor = host.read_with(cx, |host, _| host.0.clone());
        let record = |cx: &mut gpui::VisualTestContext| {
            editor.update_in(cx, |editor, window, cx| editor.record(stop(), window, cx));
        };
        record(cx);
        cx.simulate_keystrokes("space");
        editor.read_with(cx, |editor, _| {
            assert_eq!(editor.recording(), None);
            assert!(matches!(editor.note(stop()), Some(Note::Taken(..))), "Space is taken");
        });
        editor.update(cx, |editor, cx| editor.replace(stop(), cx));
        editor.read_with(cx, |editor, _| assert_eq!(editor.draft().keys(stop())[0].key, "space"));
        record(cx);
        cx.simulate_keystrokes("backspace");
        editor.read_with(cx, |editor, _| {
            assert_eq!(editor.draft().keys(stop()), []);
            assert!(editor.is_dirty());
        });
        cx.simulate_keystrokes("z");
        editor.read_with(cx, |editor, _| assert_eq!(editor.draft().keys(stop()), []));
    }
}
