//! The `bookmarks` table (S8.3.3): the places the user marked in a media, with their names.
//!
//! Rows are keyed by the media's identity (S1.7.2), the content key of a file or `url:` and a
//! URL, not by its location, so a renamed or moved file keeps its bookmarks and copies of it share
//! them. A media has at most one bookmark per millisecond: adding one where there is one already
//! changes nothing.
//!
//! Bookmarks are kept apart from the history: Clear History leaves them.

use std::time::Duration;

use rusqlite::{Connection, params};

use super::now_ms;

/// Version 12 (S8.3.3): each media's bookmarks, by identity and position.
pub(super) const MIGRATION: &str = "
    CREATE TABLE bookmarks (
        identity TEXT NOT NULL,
        position_ms INTEGER NOT NULL,
        name TEXT NOT NULL,
        created_ms INTEGER NOT NULL,
        PRIMARY KEY (identity, position_ms)
    );
";

/// A place marked in a media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bookmark {
    /// Where it is, to the millisecond.
    pub position: Duration,
    /// What the user calls it.
    pub name: String,
}

/// The bookmarks of the media with `identity`, by position.
pub fn load(connection: &Connection, identity: &str) -> rusqlite::Result<Vec<Bookmark>> {
    let mut statement = connection.prepare_cached(
        "SELECT position_ms, name FROM bookmarks WHERE identity = ?1 ORDER BY position_ms",
    )?;
    let rows = statement.query_map([identity], |row| {
        let millis: i64 = row.get(0)?;
        Ok(Bookmark { position: from_millis(millis), name: row.get(1)? })
    })?;
    rows.collect()
}

/// Adds `bookmark` to the media with `identity`; nothing when one is already at its position.
pub fn add(connection: &Connection, identity: &str, bookmark: &Bookmark) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO bookmarks (identity, position_ms, name, created_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (identity, position_ms) DO NOTHING",
        params![identity, to_millis(bookmark.position), bookmark.name, now_ms()],
    )?;
    Ok(())
}

/// Renames the bookmark at `position` in the media with `identity`.
pub fn rename(
    connection: &Connection,
    identity: &str,
    position: Duration,
    name: &str,
) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE bookmarks SET name = ?3 WHERE identity = ?1 AND position_ms = ?2",
        params![identity, to_millis(position), name],
    )?;
    Ok(())
}

/// Removes the bookmark at `position` from the media with `identity`.
pub fn remove(connection: &Connection, identity: &str, position: Duration) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM bookmarks WHERE identity = ?1 AND position_ms = ?2",
        params![identity, to_millis(position)],
    )?;
    Ok(())
}

/// A position as stored: whole milliseconds, saturating far past any media's length.
fn to_millis(position: Duration) -> i64 {
    i64::try_from(position.as_millis()).unwrap_or(i64::MAX)
}

/// A stored position; a negative one, which nothing writes, is the start.
fn from_millis(millis: i64) -> Duration {
    Duration::from_millis(u64::try_from(millis).unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn at(millis: u64, name: &str) -> Bookmark {
        Bookmark { position: Duration::from_millis(millis), name: name.into() }
    }

    #[test]
    fn bookmarks_are_listed_by_position_per_identity() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        add(connection, "a", &at(44_125, "Two")).unwrap();
        add(connection, "a", &at(42_023, "One")).unwrap();
        add(connection, "b", &at(1_000, "Other")).unwrap();
        assert_eq!(load(connection, "a").unwrap(), [at(42_023, "One"), at(44_125, "Two")]);
    }

    #[test]
    fn a_second_bookmark_at_the_same_place_is_not_added() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        add(connection, "a", &at(5, "First")).unwrap();
        add(connection, "a", &at(5, "Second")).unwrap();
        assert_eq!(load(connection, "a").unwrap(), [at(5, "First")]);
    }

    #[test]
    fn bookmarks_are_renamed_and_removed() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        add(connection, "a", &at(5, "One")).unwrap();
        add(connection, "a", &at(9, "Two")).unwrap();
        rename(connection, "a", Duration::from_millis(5), "Start").unwrap();
        remove(connection, "a", Duration::from_millis(9)).unwrap();
        assert_eq!(load(connection, "a").unwrap(), [at(5, "Start")]);
    }
}
