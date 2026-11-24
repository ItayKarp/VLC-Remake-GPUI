//! Which keys a shortcut may use (S5.6.4): not a modifier alone, not with Fn, and not a
//! combination the OS keeps for itself (Cmd+Tab, Win+L, Alt+F4). A command's own default key
//! is always allowed, so Quit keeps Cmd+Q.

use gpui::{Keystroke, OperatingSystem};

/// Why a key can't be a shortcut.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The text isn't a key in GPUI's syntax.
    Invalid,
    /// Only modifiers were pressed.
    ModifierOnly,
    /// The key was pressed with Fn, which most keyboards don't report reliably.
    Function,
    /// The OS uses the combination; the text says what for.
    Reserved(&'static str),
}

impl Refusal {
    /// What the dialog says under the key.
    pub fn message(&self) -> String {
        match self {
            Refusal::Invalid => tr!("That isn't a key VLC knows.").into(),
            Refusal::ModifierOnly => tr!("Add a key to the modifiers.").into(),
            Refusal::Function => tr!("Fn combinations can't be shortcuts.").into(),
            Refusal::Reserved(what) => reserved_message(what).into(),
        }
    }
}

/// The whole sentence for a key the OS keeps for `what`, so translators see each one.
fn reserved_message(what: &str) -> &'static str {
    match what {
        "quit apps" => tr!("The system uses this key to quit apps."),
        "switch apps" => tr!("The system uses this key to switch apps."),
        "switch windows" => tr!("The system uses this key to switch windows."),
        "open Spotlight" => tr!("The system uses this key to open Spotlight."),
        "hide apps" => tr!("The system uses this key to hide apps."),
        "minimize windows" => tr!("The system uses this key to minimize windows."),
        "lock the screen" => tr!("The system uses this key to lock the screen."),
        "force apps to quit" => tr!("The system uses this key to force apps to quit."),
        "take screenshots" => tr!("The system uses this key to take screenshots."),
        "lock the computer" => tr!("The system uses this key to lock the computer."),
        "show the desktop" => tr!("The system uses this key to show the desktop."),
        "close windows" => tr!("The system uses this key to close windows."),
        "open the security screen" => {
            tr!("The system uses this key to open the security screen.")
        }
        "open Task Manager" => tr!("The system uses this key to open Task Manager."),
        "log out" => tr!("The system uses this key to log out."),
        _ => tr!("The system uses this key."),
    }
}

/// Keys the OS handles before any app sees them, or that would break the desktop if an app
/// took them, and what the OS does with each.
fn reserved(os: OperatingSystem) -> &'static [(&'static str, &'static str)] {
    match os {
        OperatingSystem::Mac => &[
            ("cmd-q", "quit apps"),
            ("cmd-tab", "switch apps"),
            ("cmd-shift-tab", "switch apps"),
            ("cmd-`", "switch windows"),
            ("cmd-space", "open Spotlight"),
            ("cmd-h", "hide apps"),
            ("cmd-m", "minimize windows"),
            ("ctrl-cmd-q", "lock the screen"),
            ("cmd-alt-escape", "force apps to quit"),
            ("cmd-shift-3", "take screenshots"),
            ("cmd-shift-4", "take screenshots"),
            ("cmd-shift-5", "take screenshots"),
        ],
        OperatingSystem::Windows => &[
            ("win-l", "lock the computer"),
            ("win-d", "show the desktop"),
            ("alt-f4", "close windows"),
            ("alt-tab", "switch windows"),
            ("ctrl-alt-delete", "open the security screen"),
            ("ctrl-shift-escape", "open Task Manager"),
            ("printscreen", "take screenshots"),
        ],
        _ => &[
            ("super-l", "lock the screen"),
            ("alt-f4", "close windows"),
            ("alt-tab", "switch windows"),
            ("ctrl-alt-delete", "log out"),
            ("printscreen", "take screenshots"),
        ],
    }
}

/// Whether `a` and `b` are the same key with the same modifiers, whatever they type.
pub fn same_key(a: &Keystroke, b: &Keystroke) -> bool {
    a.modifiers == b.modifiers && a.key == b.key
}

/// Whether `keystroke` may be a shortcut on `os`. Default keys don't need asking.
pub fn allowed(keystroke: &Keystroke, os: OperatingSystem) -> Result<(), Refusal> {
    if keystroke.key.is_empty() || keystroke.is_modifier_key() {
        return Err(Refusal::ModifierOnly);
    }
    if keystroke.modifiers.function {
        return Err(Refusal::Function);
    }
    let taken = reserved(os).iter().find(|(source, _)| {
        Keystroke::parse_with_os(source, os).is_ok_and(|reserved| same_key(&reserved, keystroke))
    });
    taken.map_or(Ok(()), |(_, what)| Err(Refusal::Reserved(what)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `source` read as a key, if it may be a shortcut on `os`.
    fn parse(source: &str, os: OperatingSystem) -> Result<Keystroke, Refusal> {
        let keystroke = Keystroke::parse_with_os(source, os).map_err(|_| Refusal::Invalid)?;
        allowed(&keystroke, os).map(|()| keystroke)
    }

    const OSES: [OperatingSystem; 3] =
        [OperatingSystem::Mac, OperatingSystem::Windows, OperatingSystem::Linux];

    #[test]
    fn every_reserved_key_parses() {
        for os in OSES {
            for (source, _) in reserved(os) {
                assert!(Keystroke::parse_with_os(source, os).is_ok(), "{source} on {os:?}");
            }
        }
    }

    #[test]
    fn the_system_keeps_its_keys() {
        let mac = OperatingSystem::Mac;
        assert_eq!(parse("cmd-tab", mac), Err(Refusal::Reserved("switch apps")));
        assert_eq!(
            parse("alt-f4", OperatingSystem::Windows),
            Err(Refusal::Reserved("close windows"))
        );
        assert!(parse("alt-f4", mac).is_ok(), "only Windows and Linux close windows with it");
        assert!(parse("ctrl-tab", mac).is_ok());
    }

    #[test]
    fn modifiers_alone_and_fn_are_refused() {
        let os = OperatingSystem::Linux;
        assert_eq!(parse("shift", os), Err(Refusal::ModifierOnly));
        assert_eq!(parse("fn-f", os), Err(Refusal::Function));
        assert_eq!(parse("ctrl-shift", os), Err(Refusal::ModifierOnly));
        assert_eq!(parse("shift-x", os).map(|k| k.key), Ok("x".into()));
    }
}
