//! Headless tests of what turning history off offers (S1.7.5, S5.5): clearing what was saved
//! when "Save recently played items" goes off, and forgetting the saved positions when
//! "Remember the playback position" does.

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};

use crate::db::media::{self, Entry, Progress};
use crate::history::tests::support::TestHistory;
use crate::main_window::tests::{open, press_toast, toast_messages};
use crate::settings;

/// The main window with history on and `positions` media left partway.
fn with_positions(
    cx: &mut TestAppContext,
    positions: usize,
) -> (&mut VisualTestContext, TestHistory) {
    let mut history = TestHistory::install(cx);
    history.read(move |connection| {
        for at in 0..positions {
            let location = format!("https://e.com/{at}.mp4");
            let entry = Entry { identity: format!("url:{location}"), location, stamp: None };
            let id = media::insert(connection, &entry).expect("a row");
            let position = Some(Duration::from_secs(30));
            let progress = Progress { position, ..Progress::default() };
            media::save_progress(connection, id, progress, 0).expect("saved");
        }
    });
    let cx = open(cx);
    cx.update(|_, cx| {
        settings::update(cx, |s| {
            s.prefs.privacy.save_recently_played = true;
            s.prefs.privacy.remember_playback_position = true;
        });
    });
    history.settle(cx);
    (cx, history)
}

fn saved_positions(history: &mut TestHistory) -> u32 {
    history.read(|connection| media::count_positions(connection).expect("counted"))
}

#[gpui::test]
fn turning_history_off_offers_to_clear_it(cx: &mut TestAppContext) {
    let (cx, mut history) = with_positions(cx, 0);
    cx.update(|_, cx| settings::update(cx, |s| s.prefs.privacy.save_recently_played = false));
    history.settle(cx);
    assert_eq!(toast_messages(cx), ["History is off. Clear what was already saved?"]);
}

#[gpui::test]
fn turning_positions_off_offers_to_forget_the_saved_one(cx: &mut TestAppContext) {
    let (cx, mut history) = with_positions(cx, 1);
    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.privacy.remember_playback_position = false);
    });
    history.settle(cx);
    assert_eq!(
        toast_messages(cx),
        ["Positions are no longer saved. Forget the one already saved?"]
    );
    press_toast(cx, "Forget Positions");
    history.settle(cx);
    assert_eq!(saved_positions(&mut history), 0);
}

#[gpui::test]
fn several_saved_positions_are_counted(cx: &mut TestAppContext) {
    let (cx, mut history) = with_positions(cx, 2);
    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.privacy.remember_playback_position = false);
    });
    history.settle(cx);
    assert_eq!(toast_messages(cx), ["Positions are no longer saved. Forget the 2 already saved?"]);
}

#[gpui::test]
fn with_no_saved_position_nothing_is_offered(cx: &mut TestAppContext) {
    let (cx, mut history) = with_positions(cx, 0);
    cx.update(|_, cx| {
        settings::update(cx, |s| s.prefs.privacy.remember_playback_position = false);
    });
    history.settle(cx);
    assert_eq!(toast_messages(cx), Vec::<String>::new());
}
