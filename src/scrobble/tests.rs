//! The queue end to end: plays queued in the database, submitted a batch at a time through a
//! stand-in submitter, and what each verdict does to the queue and to what the user is told.

use std::collections::VecDeque;
use std::sync::Mutex;

use gpui::TestAppContext;

use super::*;
use crate::history::tests::support::TestHistory;

/// A submitter answering scripted verdicts (then [`Verdict::Sent`]), noting each batch's size.
#[derive(Default)]
struct Script {
    verdicts: Mutex<VecDeque<Verdict>>,
    batches: Mutex<Vec<usize>>,
}

impl Script {
    fn answer(&self, verdicts: impl IntoIterator<Item = Verdict>) {
        self.verdicts.lock().unwrap().extend(verdicts);
    }

    fn batches(&self) -> Vec<usize> {
        self.batches.lock().unwrap().clone()
    }

    fn submitter(self: &Arc<Self>) -> Submit {
        let script = Arc::clone(self);
        Arc::new(move |_: &Config, plays: &[Play]| {
            script.batches.lock().unwrap().push(plays.len());
            script.verdicts.lock().unwrap().pop_front().unwrap_or(Verdict::Sent)
        })
    }
}

/// Scrobbling on as "ana", through `script`, with a database on the test's thread.
fn start(script: &Arc<Script>, cx: &mut TestAppContext) -> TestHistory {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        settings::update(cx, |s| {
            s.prefs.privacy.lastfm.submit = true;
            s.prefs.privacy.lastfm.username = "ana".into();
        });
    });
    let mut history = TestHistory::install(cx);
    let submit = script.submitter();
    cx.update(|cx| init_with(submit, cx));
    history.settle(cx);
    history
}

fn play(title: &str) -> Play {
    Play {
        artist: "Björk".into(),
        title: title.into(),
        album: None,
        duration_s: None,
        started_s: crate::db::now_ms() / 1000,
    }
}

fn queued(history: &mut TestHistory) -> usize {
    let now_s = crate::db::now_ms() / 1000;
    history.read(move |connection| scrobbles::next_batch(connection, now_s, 1_000).unwrap().len())
}

fn listen(title: &str, history: &mut TestHistory, cx: &mut TestAppContext) {
    let played = play(title);
    cx.update(|cx| scrobble(played, cx));
    history.settle(cx);
}

#[gpui::test]
fn a_play_is_queued_and_sent(cx: &mut TestAppContext) {
    let script = Arc::new(Script::default());
    let mut history = start(&script, cx);
    listen("Jóga", &mut history, cx);
    assert_eq!(script.batches(), [1]);
    assert_eq!(queued(&mut history), 0);
    cx.update(|cx| assert_eq!(take_problem(cx), None));
}

#[gpui::test]
fn nothing_is_queued_while_scrobbling_is_off(cx: &mut TestAppContext) {
    let script = Arc::new(Script::default());
    let mut history = start(&script, cx);
    cx.update(|cx| settings::update(cx, |s| s.prefs.privacy.lastfm.submit = false));
    listen("Jóga", &mut history, cx);
    assert_eq!(script.batches(), Vec::<usize>::new());
    assert_eq!(queued(&mut history), 0);
}

#[gpui::test]
fn refused_plays_are_dropped(cx: &mut TestAppContext) {
    let script = Arc::new(Script::default());
    script.answer([Verdict::Refused]);
    let mut history = start(&script, cx);
    listen("Untagged", &mut history, cx);
    assert_eq!(queued(&mut history), 0);
    cx.update(|cx| assert_eq!(take_problem(cx), None, "nothing to tell"));
}

#[gpui::test]
fn offline_plays_wait_and_go_after_the_retry(cx: &mut TestAppContext) {
    let script = Arc::new(Script::default());
    script.answer([Verdict::Stopped(Problem::Offline)]);
    let mut history = start(&script, cx);
    listen("Jóga", &mut history, cx);
    assert_eq!(queued(&mut history), 1, "kept for later");
    cx.update(|cx| {
        assert_eq!(take_problem(cx), Some(Problem::Offline));
        assert_eq!(take_problem(cx), None, "told once");
    });

    cx.executor().advance_clock(RETRY);
    history.settle(cx);
    assert_eq!(script.batches(), [1, 1]);
    assert_eq!(queued(&mut history), 0);
}

#[gpui::test]
fn a_rejected_login_waits_for_the_account_to_change(cx: &mut TestAppContext) {
    let script = Arc::new(Script::default());
    script.answer([Verdict::Stopped(Problem::LoginRejected)]);
    let mut history = start(&script, cx);
    listen("Jóga", &mut history, cx);
    listen("Hunter", &mut history, cx);
    assert_eq!(script.batches(), [1], "not tried again with the same account");
    cx.update(|cx| assert_eq!(take_problem(cx), Some(Problem::LoginRejected)));

    cx.update(|cx| settings::update(cx, |s| s.prefs.privacy.lastfm.username = "bo".into()));
    history.settle(cx);
    assert_eq!(script.batches(), [1, 2]);
    assert_eq!(queued(&mut history), 0);
}

#[test]
fn every_problem_says_what_to_do() {
    let problems = [
        Problem::NotSetUp,
        Problem::NoAccount,
        Problem::LoginRejected,
        Problem::KeyRejected,
        Problem::Offline,
    ];
    for problem in problems {
        assert!(!problem.message().is_empty(), "{problem:?}");
        assert_eq!(problem.retries(), problem == Problem::Offline);
    }
}
