use super::*;
use crate::db::Database;
use crate::podcasts::feed;

fn feed(title: &str, guids: &[&str]) -> Feed {
    let episodes = guids.iter().enumerate().map(|(n, guid)| feed::Episode {
        guid: (*guid).to_owned(),
        title: format!("Episode {guid}"),
        url: format!("https://e.com/{guid}.mp3"),
        published_s: Some(1000 - n as i64),
        duration_s: Some(60),
    });
    Feed { title: title.to_owned(), episodes: episodes.collect() }
}

fn titles(connection: &Connection, id: FeedId) -> Vec<String> {
    let episodes = episodes(connection).expect("episodes").into_iter();
    episodes.filter(|episode| episode.feed == id).map(|episode| episode.title).collect()
}

#[test]
fn subscribing_lists_the_feed_and_its_episodes_latest_first() {
    let mut database = Database::in_memory();
    let connection = database.connection();
    let id = subscribe(connection, "https://e.com/feed", &feed("Show", &["a", "b"]), 1)
        .expect("subscribe");
    let again = subscribe(connection, "https://e.com/feed", &feed("Show 2", &["a"]), 2)
        .expect("subscribe again");
    assert_eq!(id, again);
    assert_eq!(
        subscriptions(connection).expect("subscriptions"),
        [Subscription { id, url: "https://e.com/feed".into(), title: "Show 2".into() }]
    );
    assert_eq!(titles(connection, id), ["Episode a", "Episode b"]);
}

#[test]
fn refreshing_keeps_played_marks_and_old_episodes() {
    let mut database = Database::in_memory();
    let connection = database.connection();
    let id = subscribe(connection, "u", &feed("Show", &["a", "b"]), 1).expect("subscribe");
    assert_eq!(played_url(connection, "https://e.com/a.mp3").expect("played"), 1);
    assert!(refresh(connection, id, &feed("Show", &["c", "a"]), 2).expect("refresh"));
    let episodes = episodes(connection).expect("episodes");
    let played: Vec<(&str, bool)> =
        episodes.iter().map(|episode| (episode.title.as_str(), episode.played)).collect();
    assert_eq!(played, [("Episode c", false), ("Episode a", true), ("Episode b", false)]);
    let first = episodes[0].id;
    set_played(connection, &[first], true).expect("mark");
    set_played(connection, &[first], false).expect("unmark");
    assert!(!super::episodes(connection).expect("episodes")[0].played);
}

#[test]
fn unsubscribing_forgets_the_episodes() {
    let mut database = Database::in_memory();
    let connection = database.connection();
    let id = subscribe(connection, "u", &feed("Show", &["a"]), 1).expect("subscribe");
    unsubscribe(connection, id).expect("unsubscribe");
    assert_eq!(subscriptions(connection).expect("subscriptions"), []);
    assert_eq!(episodes(connection).expect("episodes"), []);
    assert!(!refresh(connection, id, &feed("Show", &["a"]), 2).expect("refresh"));
}
