//! The `playlists` and `playlist_items` tables (S3.5): saved playlists, each a title, an optional
//! custom thumbnail and its items in order. Items are kept by location (a stored path or a URL),
//! like the queue in the session file, so a playlist doesn't hash its files to be saved.
//!
//! What a title may be is `playlists::title`'s job; this module only reads and writes rows.

use rusqlite::{Connection, OptionalExtension as _, params};

/// A row of the `playlists` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlaylistId(pub i64);

/// What lists of playlists show: the title and how many items it has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// The playlist.
    pub id: PlaylistId,
    /// Its title.
    pub title: String,
    /// How many items it has.
    pub len: usize,
}

/// A whole playlist: what creating, duplicating and undoing a delete write.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stored {
    /// The title.
    pub title: String,
    /// A custom thumbnail image as a stored path; `None` for the automatic one.
    pub thumbnail: Option<String>,
    /// The items' stored paths or URLs, in order.
    pub locations: Vec<String>,
}

/// Every playlist, by title (ignoring case), then oldest first.
pub fn list(connection: &Connection) -> rusqlite::Result<Vec<Summary>> {
    let mut statement = connection.prepare(
        "SELECT id, title, (SELECT COUNT(*) FROM playlist_items WHERE playlist_id = id)
         FROM playlists ORDER BY title COLLATE NOCASE, id",
    )?;
    let rows = statement.query_map([], |row| {
        let len: i64 = row.get(2)?;
        Ok(Summary { id: PlaylistId(row.get(0)?), title: row.get(1)?, len: len as usize })
    })?;
    rows.collect()
}

/// Writes `playlist` as a new row, at `id` when given (undoing a delete brings it back where it
/// was), and returns its id.
pub fn insert(
    connection: &mut Connection,
    id: Option<PlaylistId>,
    playlist: &Stored,
    now_ms: i64,
) -> rusqlite::Result<PlaylistId> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO playlists (id, title, thumbnail, created_ms, modified_ms)
         VALUES (?1, ?2, ?3, ?4, ?4)",
        params![id.map(|id| id.0), playlist.title, playlist.thumbnail, now_ms],
    )?;
    let id = PlaylistId(transaction.last_insert_rowid());
    write_items(&transaction, id, 0, &playlist.locations)?;
    transaction.commit()?;
    Ok(id)
}

/// Playlist `id` whole, if it exists.
pub fn load(connection: &Connection, id: PlaylistId) -> rusqlite::Result<Option<Stored>> {
    let head = connection
        .query_row("SELECT title, thumbnail FROM playlists WHERE id = ?1", [id.0], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let Some((title, thumbnail)) = head else { return Ok(None) };
    let mut statement = connection
        .prepare("SELECT location FROM playlist_items WHERE playlist_id = ?1 ORDER BY position")?;
    let locations = statement.query_map([id.0], |row| row.get(0))?.collect::<Result<_, _>>()?;
    Ok(Some(Stored { title, thumbnail, locations }))
}

/// Renames playlist `id`.
pub fn rename(
    connection: &Connection,
    id: PlaylistId,
    title: &str,
    now_ms: i64,
) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE playlists SET title = ?2, modified_ms = ?3 WHERE id = ?1",
        params![id.0, title, now_ms],
    )?;
    Ok(())
}

/// Adds `locations` at the end of playlist `id`.
pub fn append(
    connection: &mut Connection,
    id: PlaylistId,
    locations: &[String],
    now_ms: i64,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    let end: i64 = transaction.query_row(
        "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_items WHERE playlist_id = ?1",
        [id.0],
        |row| row.get(0),
    )?;
    write_items(&transaction, id, end, locations)?;
    transaction
        .execute("UPDATE playlists SET modified_ms = ?2 WHERE id = ?1", params![id.0, now_ms])?;
    transaction.commit()
}

/// Deletes playlist `id` and its items, and returns what it held (for Undo), if it existed.
pub fn delete(connection: &mut Connection, id: PlaylistId) -> rusqlite::Result<Option<Stored>> {
    let transaction = connection.transaction()?;
    let deleted = load(&transaction, id)?;
    transaction.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id.0])?;
    transaction.execute("DELETE FROM playlists WHERE id = ?1", [id.0])?;
    transaction.commit()?;
    Ok(deleted)
}

/// Writes `locations` as playlist `id`'s items from position `first` on.
fn write_items(
    connection: &Connection,
    id: PlaylistId,
    first: i64,
    locations: &[String],
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(
        "INSERT INTO playlist_items (playlist_id, position, location) VALUES (?1, ?2, ?3)",
    )?;
    for (position, location) in (first..).zip(locations) {
        statement.execute(params![id.0, position, location])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn stored(title: &str, locations: &[&str]) -> Stored {
        let locations = locations.iter().map(|&l| l.to_owned()).collect();
        Stored { title: title.into(), thumbnail: None, locations }
    }

    #[test]
    fn a_playlist_keeps_its_items_in_order() {
        let mut database = Database::in_memory();
        let c = database.connection();
        let playlist = Stored { thumbnail: Some("/t.png".into()), ..stored("Mix", &["/b", "/a"]) };
        let id = insert(c, None, &playlist, 1).expect("insert");
        append(c, id, &["/c".to_owned()], 2).expect("append");
        let loaded = load(c, id).expect("load").expect("it exists");
        assert_eq!(loaded.locations, ["/b", "/a", "/c"]);
        assert_eq!(loaded.thumbnail.as_deref(), Some("/t.png"));
        assert_eq!(list(c).expect("list"), [Summary { id, title: "Mix".into(), len: 3 }]);
    }

    #[test]
    fn playlists_are_listed_by_title_ignoring_case() {
        let mut database = Database::in_memory();
        let c = database.connection();
        for title in ["beta", "Alpha", "gamma"] {
            insert(c, None, &stored(title, &[]), 1).expect("insert");
        }
        let titles: Vec<_> = list(c).expect("list").into_iter().map(|s| s.title).collect();
        assert_eq!(titles, ["Alpha", "beta", "gamma"]);
    }

    #[test]
    fn a_deleted_playlist_comes_back_where_it_was() {
        let mut database = Database::in_memory();
        let c = database.connection();
        let id = insert(c, None, &stored("Mix", &["/a", "/b"]), 1).expect("insert");
        rename(c, id, "Renamed", 2).expect("rename");
        let deleted = delete(c, id).expect("delete").expect("it existed");
        assert_eq!(deleted, stored("Renamed", &["/a", "/b"]));
        assert!(load(c, id).expect("load").is_none());
        assert!(delete(c, id).expect("delete again").is_none());
        assert_eq!(insert(c, Some(id), &deleted, 3).expect("restore"), id);
        assert_eq!(load(c, id).expect("load"), Some(deleted));
    }
}
