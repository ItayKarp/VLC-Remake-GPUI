//! The `network_streams` table (S4.11): streams opened from Open Network Stream (Recent Streams)
//! and the ones the user filed under Network's built-in categories, Podcasts and Radio
//! (S4.11.3). A stream is its URL; one URL is listed at most once per category.

use rusqlite::{Connection, params};

use super::text_enum::text_enum;
use crate::navigation::route::NetworkCategory;

text_enum!(NetworkCategory { Recent => "recent", Podcasts => "podcasts", Radio => "radio" });

/// How many recent streams are kept: opening more forgets the oldest.
pub const MAX_RECENT: usize = 50;

/// One listed stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    /// The URL, as entered.
    pub url: String,
    /// The category it is listed in.
    pub category: NetworkCategory,
}

/// The streams of every category: the recent ones most recently opened first, the others in
/// the order they were added.
pub fn list(connection: &Connection) -> rusqlite::Result<Vec<Stream>> {
    let mut statement = connection.prepare(
        "SELECT url, category FROM network_streams
         ORDER BY category, COALESCE(last_played_ms, added_ms) DESC, id DESC",
    )?;
    let rows =
        statement.query_map([], |row| Ok(Stream { url: row.get(0)?, category: row.get(1)? }));
    rows?.collect()
}

/// Lists `urls` first among the recent streams, the first of them at the very top, and forgets
/// the oldest beyond [`MAX_RECENT`].
pub fn record_recent(
    connection: &mut Connection,
    urls: &[String],
    now_ms: i64,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    // The first URL gets the latest time, so it lists first.
    for (age, url) in urls.iter().rev().enumerate() {
        let at = now_ms + age as i64;
        transaction.execute(
            "INSERT INTO network_streams (url, category, added_ms, last_played_ms)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT (category, url) DO UPDATE SET last_played_ms = excluded.last_played_ms",
            params![url, NetworkCategory::Recent, at],
        )?;
    }
    transaction.execute(
        "DELETE FROM network_streams WHERE category = ?1 AND id NOT IN (
             SELECT id FROM network_streams WHERE category = ?1
             ORDER BY COALESCE(last_played_ms, added_ms) DESC, id DESC LIMIT ?2)",
        params![NetworkCategory::Recent, MAX_RECENT as i64],
    )?;
    transaction.commit()
}

/// Lists `url` in `category`, unless it is there already.
pub fn add(
    connection: &Connection,
    url: &str,
    category: NetworkCategory,
    now_ms: i64,
) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT OR IGNORE INTO network_streams (url, category, added_ms) VALUES (?1, ?2, ?3)",
        params![url, category, now_ms],
    )?;
    Ok(())
}

/// Takes `url` off `category`'s list.
pub fn remove(
    connection: &Connection,
    url: &str,
    category: NetworkCategory,
) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM network_streams WHERE url = ?1 AND category = ?2",
        params![url, category],
    )?;
    Ok(())
}

/// Forgets the recent streams (Clear History); Podcasts and Radio stay.
pub fn clear_recent(connection: &Connection) -> rusqlite::Result<()> {
    connection
        .execute("DELETE FROM network_streams WHERE category = ?1", [NetworkCategory::Recent])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn urls(connection: &Connection, category: NetworkCategory) -> Vec<String> {
        let streams = list(connection).expect("list");
        streams.into_iter().filter(|s| s.category == category).map(|s| s.url).collect()
    }

    #[test]
    fn recent_streams_list_the_latest_first_and_keep_a_few() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        let opened = ["a", "b"].map(String::from);
        record_recent(connection, &opened, 10).expect("record");
        record_recent(connection, &["c".into(), "a".into()], 20).expect("record");
        assert_eq!(urls(connection, NetworkCategory::Recent), ["c", "a", "b"]);
        let many: Vec<String> = (0..MAX_RECENT + 5).map(|n| format!("u{n}")).collect();
        record_recent(connection, &many, 30).expect("record");
        let kept = urls(connection, NetworkCategory::Recent);
        assert_eq!(kept.len(), MAX_RECENT);
        assert_eq!(kept[0], "u0");
    }

    #[test]
    fn categories_list_a_url_once_and_clearing_keeps_them() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        add(connection, "x", NetworkCategory::Radio, 1).expect("add");
        add(connection, "x", NetworkCategory::Radio, 2).expect("add");
        add(connection, "y", NetworkCategory::Radio, 3).expect("add");
        record_recent(connection, &["x".into()], 4).expect("record");
        assert_eq!(urls(connection, NetworkCategory::Radio), ["y", "x"]);
        clear_recent(connection).expect("clear");
        assert_eq!(urls(connection, NetworkCategory::Recent), Vec::<String>::new());
        remove(connection, "y", NetworkCategory::Radio).expect("remove");
        assert_eq!(urls(connection, NetworkCategory::Radio), ["x"]);
    }
}
