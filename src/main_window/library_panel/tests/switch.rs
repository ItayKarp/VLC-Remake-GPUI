//! Headless tests of "Use the media library" (S5.9.4): turning it off hides the library, which
//! then can't be shown until it is on again.

use gpui::{TestAppContext, VisualTestContext};

use super::browse;
use crate::main_window::tests::{main_view, tap};
use crate::settings;

fn is_open(cx: &mut VisualTestContext) -> bool {
    main_view(cx).read_with(cx, |main, _| main.library_panel.is_open())
}

fn set_enabled(cx: &mut VisualTestContext, enabled: bool) {
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.library.enabled = enabled));
    cx.run_until_parked();
}

#[gpui::test]
fn turning_the_library_off_hides_it_until_it_is_on_again(cx: &mut TestAppContext) {
    let cx = browse(cx, 1);
    assert!(is_open(cx));
    set_enabled(cx, false);
    assert!(!is_open(cx), "turning the library off hides it");
    tap(cx, "l");
    assert!(!is_open(cx), "L doesn't show it while it is off");
    set_enabled(cx, true);
    tap(cx, "l");
    assert!(is_open(cx), "L shows it again once it is on");
}
