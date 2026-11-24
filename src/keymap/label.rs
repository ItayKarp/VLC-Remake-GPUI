//! How shortcuts are written for people: "Ctrl + O" on Windows and Linux, "⌘O" on
//! macOS (menus, tooltips), and as separate key caps (onboarding, the shortcuts
//! dialog).

use gpui::{Keystroke, Modifiers, SharedString};

/// The direction of an arrow key, drawn as an icon on a key cap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    /// ←
    Left,
    /// →
    Right,
    /// ↑
    Up,
    /// ↓
    Down,
}

/// One key cap: a modifier or key name, or an arrow icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyCap {
    /// A name such as "SHIFT", "Space" or "F".
    Text(SharedString),
    /// An arrow key.
    Arrow(Arrow),
}

/// `keystroke` as one line of text, e.g. "Ctrl + Shift + O" or "⇧⌘O".
pub fn text(keystroke: &Keystroke, mac: bool) -> String {
    let key = key_name(&keystroke.key, mac);
    if mac {
        let mut out: String = modifier_names(keystroke.modifiers, mac).concat();
        out.push_str(&key);
        return out;
    }
    let mut parts = modifier_names(keystroke.modifiers, mac);
    parts.push(&key);
    parts.join(" + ")
}

/// `keystroke` as key caps: one per modifier, then the key.
pub fn caps(keystroke: &Keystroke, mac: bool) -> Vec<KeyCap> {
    let mut caps: Vec<KeyCap> = modifier_names(keystroke.modifiers, mac)
        .into_iter()
        .map(|name| KeyCap::Text(name.to_uppercase().into()))
        .collect();
    caps.push(match arrow(&keystroke.key) {
        Some(direction) => KeyCap::Arrow(direction),
        None => KeyCap::Text(key_name(&keystroke.key, mac).into()),
    });
    caps
}

/// The held modifiers in each platform's conventional order.
fn modifier_names(modifiers: Modifiers, mac: bool) -> Vec<&'static str> {
    // macOS orders them ⌃⌥⇧⌘; Windows and Linux Ctrl, Alt, Shift, Win.
    let order: [(bool, &str, &str); 4] = [
        (modifiers.control, "⌃", tr!("Ctrl")),
        (modifiers.alt, "⌥", tr!("Alt")),
        (modifiers.shift, "⇧", tr!("Shift")),
        (modifiers.platform, "⌘", platform_key_name()),
    ];
    order
        .into_iter()
        .filter(|(held, ..)| *held)
        .map(|(_, symbol, name)| if mac { symbol } else { name })
        .collect()
}

/// The Windows or Super key's name.
fn platform_key_name() -> &'static str {
    if cfg!(target_os = "linux") { tr!("Super") } else { tr!("Win") }
}

fn arrow(key: &str) -> Option<Arrow> {
    match key {
        "left" => Some(Arrow::Left),
        "right" => Some(Arrow::Right),
        "up" => Some(Arrow::Up),
        "down" => Some(Arrow::Down),
        _ => None,
    }
}

/// A key's printed name: "Space", "Esc", "F1", "←", "O", "<".
fn key_name(key: &str, mac: bool) -> String {
    let named = match key {
        "space" => trc!("key", "Space"),
        "escape" => trc!("key", "Esc"),
        "enter" if mac => "↩",
        "enter" => trc!("key", "Enter"),
        "backspace" if mac => "⌫",
        "backspace" => trc!("key", "Backspace"),
        "delete" => trc!("key", "Delete"),
        "tab" => trc!("key", "Tab"),
        "left" => "←",
        "right" => "→",
        "up" => "↑",
        "down" => "↓",
        "pageup" => trc!("key", "Page Up"),
        "pagedown" => trc!("key", "Page Down"),
        "home" => trc!("key", "Home"),
        "end" => trc!("key", "End"),
        _ => return key.to_uppercase(),
    };
    named.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &'static str) -> Keystroke {
        Keystroke::parse(source).expect("test keystrokes are valid")
    }

    #[test]
    fn windows_and_linux_spell_out_modifiers() {
        assert_eq!(text(&parse("ctrl-o"), false), "Ctrl + O");
        assert_eq!(text(&parse("ctrl-shift-o"), false), "Ctrl + Shift + O");
        assert_eq!(text(&parse("shift-f1"), false), "Shift + F1");
        assert_eq!(text(&parse("space"), false), "Space");
        assert_eq!(text(&parse("<"), false), "<");
    }

    #[test]
    fn macos_uses_symbols_in_order() {
        assert_eq!(text(&parse("cmd-o"), true), "⌘O");
        assert_eq!(text(&parse("cmd-shift-o"), true), "⇧⌘O");
        assert_eq!(text(&parse("cmd-,"), true), "⌘,");
    }

    #[test]
    fn caps_split_modifiers_and_draw_arrows() {
        assert_eq!(
            caps(&parse("shift-s"), false),
            vec![KeyCap::Text("SHIFT".into()), KeyCap::Text("S".into())]
        );
        assert_eq!(caps(&parse("left"), false), vec![KeyCap::Arrow(Arrow::Left)]);
        assert_eq!(caps(&parse("space"), false), vec![KeyCap::Text("Space".into())]);
    }
}
