use gpui::{TestAppContext, VisualTestContext};

use super::*;
use crate::main_window::tests::{main_view, open};

fn effective(key: &str, cx: &mut VisualTestContext) -> &'static str {
    cx.update(|_, cx| choice(key, cx).map(|(_, chosen)| chosen.id)).expect("a choice")
}

#[gpui::test]
fn zoom_steps_through_the_choices_and_comes_back_to_fit(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    assert_eq!(effective(ZOOM, cx), "fit");
    main.update(cx, MainWindow::step_session_zoom);
    assert_ne!(effective(ZOOM, cx), "fit");
    let count = choices(ZOOM).expect("Zoom's choices").len();
    for _ in 1..count {
        main.update(cx, MainWindow::step_session_zoom);
    }
    assert_eq!(effective(ZOOM, cx), "fit");
    let saved = cx.update(|_, cx| crate::settings::get(cx).prefs.video.zoom);
    assert_eq!(saved, crate::video::framing::Zoom::Fit, "only the session changes");
}

#[gpui::test]
fn hdr_switches_between_auto_and_off(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    main.update(cx, MainWindow::toggle_session_hdr);
    assert_eq!(effective(HDR, cx), HDR_OFF);
    main.update(cx, MainWindow::toggle_session_hdr);
    assert_eq!(effective(HDR, cx), HDR_AUTO);
}

#[gpui::test]
fn favorite_waits_for_the_library_and_the_others_need_video(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    main.read_with(cx, |main, cx| {
        let enabled = |item| main.extra_button(item, cx).expect("a button").enabled;
        assert!(!enabled(Item::Favorite), "nothing plays");
        assert!(!enabled(Item::Hdr));
        assert!(!enabled(Item::Zoom));
        assert!(enabled(Item::RightSidebar));
    });
}

#[gpui::test]
fn right_sidebar_opens_the_queue_panel_on_its_last_tab(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.toggle_list_tab(crate::main_window::queue_panel::ListTab::Chapters, window, cx);
        main.toggle_right_sidebar(window, cx);
        assert!(!main.queue_panel.is_open());
        main.toggle_right_sidebar(window, cx);
        assert!(main.queue_panel.shows(crate::main_window::queue_panel::ListTab::Chapters));
    });
}
