//! Keyboard layouts (S0.5.5): the keystrokes GPUI's Windows backend reports for
//! each layout, matched against the default player bindings. On Windows, GPUI
//! names letters by their virtual key (Latin on every layout), and turns a shifted
//! symbol key into the symbol itself without Shift (Shift+`,` on US is `<`).

use gpui::{KeybindingKeystroke, Keystroke, Modifiers};

use crate::keymap::commands::{Area, COMMANDS};

/// A key press as GPUI reports it: the key, Shift, and the typed character.
fn typed(key: &str, shift: bool, character: &str) -> Keystroke {
    Keystroke {
        modifiers: Modifiers { shift, ..Modifiers::none() },
        key: key.to_owned(),
        key_char: Some(character.to_owned()),
    }
}

/// The labels of the player commands `keystroke` triggers on Windows/Linux keys.
fn triggered(keystroke: &Keystroke) -> Vec<&'static str> {
    COMMANDS
        .iter()
        .filter(|command| command.area == Area::Player)
        .filter(|command| {
            command.keys.for_os(false).iter().any(|source| {
                let Ok(target) = Keystroke::parse(source) else {
                    return false;
                };
                keystroke.should_match(&KeybindingKeystroke::from_keystroke(target))
            })
        })
        .map(|command| command.label)
        .collect()
}

fn assert_triggers(layout: &str, keystroke: &Keystroke, label: &str) {
    assert_eq!(triggered(keystroke), vec![label], "{layout}: {keystroke:?}");
}

#[test]
fn us_and_uk() {
    for layout in ["US", "UK"] {
        assert_triggers(layout, &typed("<", false, "<"), "Previous item");
        assert_triggers(layout, &typed(">", false, ">"), "Next item");
        assert_triggers(layout, &typed(",", false, ","), "Previous frame");
        assert_triggers(layout, &typed(".", false, "."), "Next frame");
        assert_triggers(layout, &typed("n", false, "n"), "Next item");
        assert_triggers(layout, &typed("s", true, "S"), "Take a snapshot");
    }
}

#[test]
fn caps_lock_s_stops_instead_of_taking_a_snapshot() {
    assert_triggers("US", &typed("s", false, "S"), "Stop playback");
}

#[test]
fn german_iso_has_its_own_angle_bracket_key() {
    // `<` is the key left of Y (VK_OEM_102); `>` is Shift on it.
    assert_triggers("German", &typed("<", false, "<"), "Previous item");
    assert_triggers("German", &typed(">", false, ">"), "Next item");
    assert_triggers("German", &typed(",", false, ","), "Previous frame");
    assert_triggers("German", &typed(".", false, "."), "Next frame");
}

#[test]
fn french_azerty_moves_the_period_to_shift() {
    assert_triggers("AZERTY", &typed(",", false, ","), "Previous frame");
    // Shift+`;` types `.`; GPUI reports it as `.` without Shift.
    assert_triggers("AZERTY", &typed(".", false, "."), "Next frame");
    assert_triggers("AZERTY", &typed("q", false, "q"), "Show / hide queue");
}

#[test]
fn hebrew_and_russian_letters_still_work() {
    assert_triggers("Hebrew", &typed("f", false, "כ"), "Toggle fullscreen");
    assert_triggers("Hebrew", &typed("l", false, "ך"), "Show / hide file browser");
    assert_triggers("Russian", &typed("f", false, "а"), "Toggle fullscreen");
    assert_triggers("Russian", &typed("v", false, "м"), "Cycle subtitle track");
    assert_triggers("Russian", &typed("p", false, "з"), "Previous item");
    // Russian `.` is unshifted on the `/` key; `,` is Shift on it.
    assert_triggers("Russian", &typed(".", false, "."), "Next frame");
    assert_triggers("Russian", &typed(",", false, ","), "Previous frame");
}
