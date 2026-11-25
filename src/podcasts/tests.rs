//! Podcasts end to end: subscribing, refreshing, unsubscribing and marking played, with feeds
//! from a stand-in [`FeedSource`] and the database on the test's thread.

use std::collections::HashMap;
use std::sync::Mutex;

use futures::FutureExt as _;
use gpui::TestAppContext;
use url::Url;

use super::feed::Episode as FeedEpisode;
use super::fetch::Failure;
use super::*;
use crate::history::tests::support::TestHistory;

const ADDRESS: &str = "https://e.com/rss";

/// Serves each address's feed, or why it can't, as set.
#[derive(Default)]
struct Stand {
    feeds: Mutex<HashMap<String, Result<Feed, Failure>>>,
}

impl Stand {
    fn serve(&self, url: &str, feed: Result<Feed, Failure>) {
        self.feeds.lock().unwrap().insert(url.to_owned(), feed);
    }
}

impl FeedSource for Stand {
    fn fetch(&self, url: &str, _: Option<&Url>) -> Result<Feed, Failure> {
        let feeds = self.feeds.lock().unwrap();
        feeds.get(url).cloned().unwrap_or(Err(Failure::Status(404)))
    }
}

/// A show with an episode for each of `guids`.
fn show(guids: &[&str]) -> Feed {
    let episode = |guid: &&str| FeedEpisode {
        guid: (*guid).to_owned(),
        title: format!("Episode {guid}"),
        url: format!("https://e.com/{guid}.mp3"),
        ..FeedEpisode::default()
    };
    Feed { title: "The Show".into(), episodes: guids.iter().map(episode).collect() }
}

/// Podcasts on `stand`, with a database on the test's thread.
fn start(stand: &Arc<Stand>, cx: &mut TestAppContext) -> TestHistory {
    cx.update(settings::init_in_memory);
    let mut history = TestHistory::install(cx);
    let source: Arc<dyn FeedSource> = stand.clone();
    cx.update(|cx| init_with(source, cx));
    history.settle(cx);
    history
}

/// Runs `task` to its end and answers what it did.
fn finish<T: 'static>(task: Task<T>, history: &mut TestHistory, cx: &mut TestAppContext) -> T {
    history.settle(cx);
    task.now_or_never().expect("the task finished once the app settled")
}

fn subscribed(stand: &Arc<Stand>, cx: &mut TestAppContext) -> (TestHistory, FeedId) {
    stand.serve(ADDRESS, Ok(show(&["1"])));
    let mut history = start(stand, cx);
    let task = cx.update(|cx| subscribe("feed://e.com/rss", cx));
    let id = finish(task, &mut history, cx).expect("subscribed");
    (history, id)
}

fn titles(id: FeedId, cx: &mut TestAppContext) -> Vec<String> {
    cx.update(|cx| episodes(id, cx).map(|episode| episode.title.clone()).collect())
}

#[gpui::test]
fn subscribing_lists_the_show_and_its_episodes(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    let (_history, id) = subscribed(&stand, cx);
    cx.update(|cx| {
        let feed = subscription(id, cx).expect("listed");
        assert_eq!((feed.url.as_str(), feed.title.as_str()), (ADDRESS, "The Show"));
        assert_eq!(subscriptions(cx).len(), 1);
    });
    assert_eq!(titles(id, cx), ["Episode 1"]);
}

#[gpui::test]
fn what_isnt_a_readable_feed_is_not_subscribed(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    let mut history = start(&stand, cx);
    let typed = cx.update(|cx| subscribe("ftp://e.com/rss", cx));
    assert_eq!(finish(typed, &mut history, cx), Err(Failure::NotALink));
    let missing = cx.update(|cx| subscribe(ADDRESS, cx));
    assert_eq!(finish(missing, &mut history, cx), Err(Failure::Status(404)));
    cx.update(|cx| assert_eq!(subscriptions(cx), []));
}

#[gpui::test]
fn without_the_library_nothing_is_subscribed(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    stand.serve(ADDRESS, Ok(show(&["1"])));
    cx.update(settings::init_in_memory);
    let task = cx.update(|cx| subscribe(ADDRESS, cx));
    cx.run_until_parked();
    let failed = task.now_or_never().expect("finished");
    assert!(matches!(failed, Err(Failure::Network(_))), "{failed:?}");
}

#[gpui::test]
fn refreshing_takes_in_new_episodes(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    let (mut history, id) = subscribed(&stand, cx);
    stand.serve(ADDRESS, Ok(show(&["1", "2"])));
    let task = cx.update(|cx| refresh(id, cx));
    assert_eq!(finish(task, &mut history, cx), Ok(()));
    assert_eq!(titles(id, cx), ["Episode 1", "Episode 2"]);

    stand.serve(ADDRESS, Err(Failure::NotAFeed));
    let task = cx.update(|cx| refresh(id, cx));
    assert_eq!(finish(task, &mut history, cx), Err(Failure::NotAFeed));
    assert_eq!(titles(id, cx).len(), 2, "a failed refresh keeps what was listed");
}

#[gpui::test]
fn starting_again_refreshes_every_feed(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    let (mut history, id) = subscribed(&stand, cx);
    stand.serve(ADDRESS, Ok(show(&["1", "2", "3"])));
    let source: Arc<dyn FeedSource> = stand.clone();
    cx.update(|cx| init_with(source, cx));
    history.settle(cx);
    assert_eq!(titles(id, cx).len(), 3);
}

#[gpui::test]
fn episodes_are_marked_played_by_hand_or_by_playing_to_the_end(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    let (mut history, id) = subscribed(&stand, cx);
    stand.serve(ADDRESS, Ok(show(&["1", "2"])));
    let task = cx.update(|cx| refresh(id, cx));
    finish(task, &mut history, cx).expect("refreshed");
    let played = |cx: &mut TestAppContext| -> Vec<bool> {
        cx.update(|cx| episodes(id, cx).map(|episode| episode.played).collect())
    };
    let first = cx.update(|cx| episodes(id, cx).next().map(|episode| episode.id)).unwrap();
    cx.update(|cx| set_played(vec![first], true, cx));
    history.settle(cx);
    assert_eq!(played(cx), [true, false]);

    cx.update(|cx| played_to_end("https://e.com/elsewhere.mp3", cx));
    cx.update(|cx| played_to_end("https://e.com/2.mp3", cx));
    history.settle(cx);
    assert_eq!(played(cx), [true, true]);
    cx.update(|cx| assert_eq!(episode(first, cx).map(|e| e.played), Some(true)));
}

#[gpui::test]
fn unsubscribing_forgets_the_show_and_its_episodes(cx: &mut TestAppContext) {
    let stand = Arc::new(Stand::default());
    let (mut history, id) = subscribed(&stand, cx);
    cx.update(|cx| unsubscribe(id, cx));
    history.settle(cx);
    cx.update(|cx| assert!(subscription(id, cx).is_none()));
    assert_eq!(titles(id, cx), Vec::<String>::new());
    let rows = history.read(|connection| podcasts::episodes(connection).unwrap().len());
    assert_eq!(rows, 0);
}
