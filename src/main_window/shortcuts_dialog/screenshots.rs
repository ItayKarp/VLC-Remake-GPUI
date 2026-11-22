//! A screenshot test of the Keyboard Shortcuts dialog against
//! `dialogs/keyboard_shortcuts/keyboard_shortcuts.png`.

use gpui::OperatingSystem;

use gpui::AppContext as _;

use super::editor::ShortcutsEditor;
use crate::keymap::custom::Shortcuts;
use crate::screenshot_test::{self, Shot};

/// The dialog inside the design PNG's drop shadow, in the PNG's pixels and in logical pixels.
const DESIGN_ORIGIN: (u32, u32) = (28, 13);
const DESIGN_SIZE: (f32, f32) = (1050., 742.);
/// The share of pixels allowed to differ: the text, the rows the design doesn't list (Hide
/// interface and the rest of Basic), the Global Hotkey wells without pencils until S5.6.7, the
/// title and search bands the design draws darker, and Save drawn as an accent button. Measured
/// near 2.6% on macOS (November 2026).
const MAX_MISMATCH: f64 = 0.04;

#[test]
fn the_dialog_matches_the_design() {
    let shot = Shot {
        name: "keyboard-shortcuts",
        design: "docs/ui/dialogs/keyboard_shortcuts/keyboard_shortcuts.png".into(),
        design_origin: DESIGN_ORIGIN,
        window: DESIGN_SIZE,
        skip_top: 0.,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        let editor = cx.new(|_| ShortcutsEditor::new(Shortcuts::defaults(OperatingSystem::Mac)));
        super::build(editor, window, cx)
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}
