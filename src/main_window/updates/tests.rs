//! The updater's news as the main window tells it: what each toast says and offers.

use gpui::TestAppContext;
use semver::Version;
use url::Url;

use crate::main_window::tests::{main_view, open};
use crate::main_window::tests::{press_toast, toasts};
use crate::updates::{News, Offer};

fn offer(version: &str) -> Offer {
    let notes_url = Url::parse("https://example.com/releases/2.0.0").unwrap();
    Offer { version: Version::parse(version).unwrap(), notes_url }
}

fn tell(cx: &mut gpui::VisualTestContext, news: News) {
    let main = main_view(cx);
    main.update(cx, |main, cx| main.tell_update_news(news, cx));
}

#[gpui::test]
fn a_readied_update_offers_its_notes_and_skipping(cx: &mut TestAppContext) {
    let cx = open(cx);
    tell(cx, News::Ready(offer("2.0.0")));
    let buttons = vec!["What's New".to_owned(), "Skip This Version".to_owned()];
    let message = "VLC 2.0.0 is ready. It installs when you quit.".to_owned();
    assert_eq!(toasts(cx), [(message, buttons)]);

    press_toast(cx, "What's New");
    assert_eq!(cx.opened_url().as_deref(), Some("https://example.com/releases/2.0.0"));
    assert_eq!(toasts(cx), Vec::new(), "the toast goes once used");
}

#[gpui::test]
fn an_update_that_cant_install_here_is_only_available(cx: &mut TestAppContext) {
    let cx = open(cx);
    tell(cx, News::Available(offer("2.1.0")));
    assert_eq!(toasts(cx)[0].0, "VLC 2.1.0 is available.");
    // Without an updater there is nothing to remember the skip in; the toast still goes.
    press_toast(cx, "Skip This Version");
    assert_eq!(toasts(cx), Vec::new());
}

#[gpui::test]
fn a_rollback_says_which_version_failed(cx: &mut TestAppContext) {
    let cx = open(cx);
    tell(cx, News::RolledBack(Version::parse("2.0.0").unwrap()));
    let message = "VLC 2.0.0 didn't start, so this version was put back.".to_owned();
    assert_eq!(toasts(cx), [(message, Vec::new())]);
}
