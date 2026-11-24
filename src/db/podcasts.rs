//! The `podcast_feeds` and `podcast_episodes` tables (S8.17.2): the feeds subscribed to in
//! Network → Podcasts and the episodes each listed, with whether each was played. Refreshing a
//! feed adds its new episodes and updates the others, keeping their played marks; episodes a
//! feed stops listing stay, as most feeds only list their latest.

use rusqlite::{Connection, Row, params};

use crate::podcasts::feed::Feed;

/// Version 14 (S8.17.2): podcast subscriptions.
pub(super) const MIGRATION: &str = "
    CREATE TABLE podcast_feeds (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL UNIQUE,
        title TEXT NOT NULL,
        added_ms INTEGER NOT NULL,
        refreshed_ms INTEGER
    );
    CREATE TABLE podcast_episodes (
        id INTEGER PRIMARY KEY,
        feed_id INTEGER NOT NULL REFERENCES podcast_feeds (id) ON DELETE CASCADE,
        guid TEXT NOT NULL,
        title TEXT NOT NULL,
        url TEXT NOT NULL,
        published_s INTEGER,
        duration_s INTEGER,
        played INTEGER NOT NULL DEFAULT 0,
        UNIQUE (feed_id, guid)
    );
    CREATE INDEX podcast_episodes_url ON podcast_episodes (url);
";

/// A subscribed feed's row id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FeedId(pub i64);

/// An episode's row id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EpisodeId(pub i64);

/// A subscribed feed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subscription {
    /// Its row.
    pub id: FeedId,
    /// The feed's address.
    pub url: String,
    /// The show's title.
    pub title: String,
}

/// An episode of a subscribed feed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Episode {
    /// Its row.
    pub id: EpisodeId,
    /// The feed listing it.
    pub feed: FeedId,
    /// Its title.
    pub title: String,
    /// The media file to play.
    pub url: String,
    /// When it was published, in seconds since the Unix epoch.
    pub published_s: Option<i64>,
    /// Its length in seconds.
    pub duration_s: Option<u32>,
    /// Whether it was played (or marked played).
    pub played: bool,
}

/// Subscribes to the feed at `url`, which said `feed`; subscribing again refreshes it. Answers
/// its id.
pub fn subscribe(
    connection: &mut Connection,
    url: &str,
    feed: &Feed,
    now_ms: i64,
) -> rusqlite::Result<FeedId> {
    let transaction = connection.transaction()?;
    let id = transaction.query_row(
        "INSERT INTO podcast_feeds (url, title, added_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT (url) DO UPDATE SET title = excluded.title
         RETURNING id",
        params![url, feed.title, now_ms],
        |row| row.get(0).map(FeedId),
    )?;
    store(&transaction, id, feed, now_ms)?;
    transaction.commit()?;
    Ok(id)
}

/// Takes in what feed `id` says now; `false` when it was unsubscribed meanwhile.
pub fn refresh(
    connection: &mut Connection,
    id: FeedId,
    feed: &Feed,
    now_ms: i64,
) -> rusqlite::Result<bool> {
    let transaction = connection.transaction()?;
    let found = transaction
        .execute("UPDATE podcast_feeds SET title = ?2 WHERE id = ?1", params![id.0, feed.title])?;
    if found == 0 {
        return Ok(false);
    }
    store(&transaction, id, feed, now_ms)?;
    transaction.commit()?;
    Ok(true)
}

/// Adds `feed`'s new episodes to feed `id`, updates the others, and notes the refresh.
fn store(connection: &Connection, id: FeedId, feed: &Feed, now_ms: i64) -> rusqlite::Result<()> {
    let mut insert = connection.prepare(
        "INSERT INTO podcast_episodes (feed_id, guid, title, url, published_s, duration_s)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (feed_id, guid) DO UPDATE SET title = excluded.title, url = excluded.url,
             published_s = excluded.published_s, duration_s = excluded.duration_s",
    )?;
    for episode in &feed.episodes {
        insert.execute(params![
            id.0,
            episode.guid,
            episode.title,
            episode.url,
            episode.published_s,
            episode.duration_s
        ])?;
    }
    connection.execute(
        "UPDATE podcast_feeds SET refreshed_ms = ?2 WHERE id = ?1",
        params![id.0, now_ms],
    )?;
    Ok(())
}

/// Unsubscribes from feed `id`, forgetting its episodes.
pub fn unsubscribe(connection: &Connection, id: FeedId) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM podcast_feeds WHERE id = ?1", [id.0])?;
    Ok(())
}

/// The feeds subscribed to, by title.
pub fn subscriptions(connection: &Connection) -> rusqlite::Result<Vec<Subscription>> {
    let mut statement = connection
        .prepare("SELECT id, url, title FROM podcast_feeds ORDER BY title COLLATE NOCASE, id")?;
    let rows = statement.query_map([], |row| {
        Ok(Subscription { id: FeedId(row.get(0)?), url: row.get(1)?, title: row.get(2)? })
    });
    rows?.collect()
}

/// Every feed's episodes, the latest first.
pub fn episodes(connection: &Connection) -> rusqlite::Result<Vec<Episode>> {
    let mut statement = connection.prepare(
        "SELECT id, feed_id, title, url, published_s, duration_s, played FROM podcast_episodes
         ORDER BY feed_id, published_s IS NULL, published_s DESC, id",
    )?;
    let rows = statement.query_map([], episode);
    rows?.collect()
}

fn episode(row: &Row) -> rusqlite::Result<Episode> {
    Ok(Episode {
        id: EpisodeId(row.get(0)?),
        feed: FeedId(row.get(1)?),
        title: row.get(2)?,
        url: row.get(3)?,
        published_s: row.get(4)?,
        duration_s: row.get(5)?,
        played: row.get(6)?,
    })
}

/// Marks episodes `ids` played or not.
pub fn set_played(
    connection: &mut Connection,
    ids: &[EpisodeId],
    played: bool,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    for id in ids {
        transaction.execute(
            "UPDATE podcast_episodes SET played = ?2 WHERE id = ?1",
            params![id.0, played],
        )?;
    }
    transaction.commit()
}

/// Marks the episodes whose file is `url` played; answers how many there were.
pub fn played_url(connection: &Connection, url: &str) -> rusqlite::Result<usize> {
    connection
        .execute("UPDATE podcast_episodes SET played = 1 WHERE url = ?1 AND played = 0", [url])
}

#[cfg(test)]
mod tests;
