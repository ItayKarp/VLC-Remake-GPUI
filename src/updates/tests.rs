//! The updater's own rules: when a check is due, telling each version once, and skipping one.

use gpui::TestAppContext;

use super::*;

fn version(text: &str) -> Version {
    Version::parse(text).unwrap()
}

fn offer(version_text: &str) -> Offer {
    let notes_url = Url::parse("https://example.com/notes").unwrap();
    Offer { version: version(version_text), notes_url }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rvlc-updater-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn updater(data_dir: Option<PathBuf>) -> Updater {
    Updater {
        status: Status::Idle,
        news: None,
        announced: None,
        installation: Installation::Msi,
        data_dir,
        last_check_s: None,
        enabled: true,
    }
}

#[test]
fn a_check_is_due_once_a_day_while_checking_is_on() {
    let mut updater = updater(None);
    assert!(updater.is_due(), "never checked");
    updater.last_check_s = Some(state::now_s() - 60);
    assert!(!updater.is_due(), "checked a minute ago");
    updater.last_check_s = Some(state::now_s() - CHECK_INTERVAL_S - 1);
    assert!(updater.is_due());
    updater.enabled = false;
    assert!(!updater.is_due(), "checking is off");
}

#[test]
fn each_version_is_told_about_once() {
    let mut updater = updater(None);
    updater.announce(News::Available(offer("2.0.0")));
    assert_eq!(updater.news.take(), Some(News::Available(offer("2.0.0"))));
    updater.announce(News::Ready(offer("2.0.0")));
    assert_eq!(updater.news, None, "the same version again");
    updater.announce(News::RolledBack(version("1.0.0")));
    assert_eq!(updater.news.take(), Some(News::RolledBack(version("1.0.0"))));
    updater.announce(News::Ready(offer("2.0.0")));
    assert_eq!(updater.news, None, "a rollback doesn't make it new");
    updater.announce(News::Ready(offer("3.0.0")));
    assert_eq!(updater.news, Some(News::Ready(offer("3.0.0"))));
}

#[gpui::test]
fn news_is_taken_once(cx: &mut TestAppContext) {
    cx.update(|cx| {
        assert_eq!(take_news(cx), None, "no updater yet");
        let mut held = updater(None);
        held.news = Some(News::Available(offer("2.0.0")));
        cx.set_global(held);
        assert_eq!(take_news(cx), Some(News::Available(offer("2.0.0"))));
        assert_eq!(take_news(cx), None);
    });
}

#[gpui::test]
fn skipping_a_readied_version_throws_it_away(cx: &mut TestAppContext) {
    let dir = temp_dir("skip");
    let package = dir.join("vlc-2.0.0.msi");
    std::fs::write(&package, "msi").unwrap();
    cx.update(|cx| {
        let mut held = updater(Some(dir.clone()));
        held.status =
            Status::Ready { version: version("2.0.0"), staged: Staged::Msi(package.clone()) };
        cx.set_global(held);
        skip(&version("2.0.0"), cx);
        assert_eq!(cx.global::<Updater>().status, Status::Idle);
    });
    assert!(!package.exists());
    assert_eq!(UpdateState::load(&dir).skipped, Some(version("2.0.0")));
}

#[gpui::test]
fn skipping_keeps_the_newest_skipped_version(cx: &mut TestAppContext) {
    let dir = temp_dir("skip-older");
    cx.update(|cx| {
        cx.set_global(updater(Some(dir.clone())));
        skip(&version("3.0.0"), cx);
        skip(&version("2.0.0"), cx);
    });
    assert_eq!(UpdateState::load(&dir).skipped, Some(version("3.0.0")));
}

#[gpui::test]
fn turning_checking_off_is_noted(cx: &mut TestAppContext) {
    cx.update(|cx| {
        settings::init_in_memory(cx);
        let mut held = updater(None);
        held.enabled = true;
        cx.set_global(held);
        follow_setting(cx);
        assert!(!cx.global::<Updater>().enabled, "the setting is off by default");
    });
}
