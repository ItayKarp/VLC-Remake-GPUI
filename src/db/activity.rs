//! What the user did in the library (S4.1), for Home (S4.9): plays (Listen Again, Frequently
//! Listened Artists), visits to folders and playlists (Frequently Visited), and favorites.
//!
//! A play is written by [`media::record_play`](super::media::record_play); Clear History
//! forgets plays and visits, but not favorites.

use rusqlite::{Connection, params};

use super::folders::FolderId;
use super::media::MediaId;
use super::metadata::MediaKind;
use super::playlists::PlaylistId;

/// A place Home counts visits to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Place {
    /// A library folder.
    Folder(FolderId),
    /// A saved playlist.
    Playlist(PlaylistId),
}

/// Something that can be a favorite.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Favorite {
    /// A media (the file menu's Favorite).
    Media(MediaId),
    /// A saved playlist (the playlist menu's Favorite).
    Playlist(PlaylistId),
}

impl Place {
    /// The `visits` column naming it, and its id.
    fn column(self) -> (&'static str, i64) {
        match self {
            Self::Folder(id) => ("folder_id", id.0),
            Self::Playlist(id) => ("playlist_id", id.0),
        }
    }
}

impl Favorite {
    /// The `favorites` column naming it, and its id.
    fn column(self) -> (&'static str, i64) {
        match self {
            Self::Media(id) => ("media_id", id.0),
            Self::Playlist(id) => ("playlist_id", id.0),
        }
    }
}

/// Counts a visit to `place` at `now_ms`.
pub fn record_visit(connection: &Connection, place: Place, now_ms: i64) -> rusqlite::Result<()> {
    let (column, id) = place.column();
    connection.execute(
        &format!(
            "INSERT INTO visits ({column}, count, last_visited_ms) VALUES (?1, 1, ?2)
             ON CONFLICT ({column}) DO UPDATE SET count = count + 1, last_visited_ms = ?2"
        ),
        params![id, now_ms],
    )?;
    Ok(())
}

/// The `limit` folders visited most, the most recent first among equals.
pub fn frequent_folders(connection: &Connection, limit: usize) -> rusqlite::Result<Vec<FolderId>> {
    frequent(connection, "folder_id", limit).map(|ids| ids.into_iter().map(FolderId).collect())
}

/// The `limit` playlists visited most, the most recent first among equals.
pub fn frequent_playlists(
    connection: &Connection,
    limit: usize,
) -> rusqlite::Result<Vec<PlaylistId>> {
    frequent(connection, "playlist_id", limit).map(|ids| ids.into_iter().map(PlaylistId).collect())
}

/// The `limit` media of `kind` played most recently, each once (Listen Again).
pub fn recently_played(
    connection: &Connection,
    kind: MediaKind,
    limit: usize,
) -> rusqlite::Result<Vec<MediaId>> {
    let mut statement = connection.prepare(
        "SELECT media.id FROM plays JOIN media ON media.id = plays.media_id
         WHERE media.kind = ?1 GROUP BY media.id ORDER BY MAX(plays.played_ms) DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(params![kind, limit as i64], |row| row.get(0).map(MediaId))?;
    rows.collect()
}

/// The `limit` artists played most (Frequently Listened Artists), by album artist when tagged.
pub fn frequent_artists(connection: &Connection, limit: usize) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT COALESCE(media.album_artist, media.artist) AS name FROM plays
         JOIN media ON media.id = plays.media_id
         WHERE media.kind = 'audio' AND name IS NOT NULL
         GROUP BY name ORDER BY COUNT(*) DESC, MAX(plays.played_ms) DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit as i64], |row| row.get(0))?;
    rows.collect()
}

/// Makes `favorite` a favorite, from `now_ms`; already one, it stays as it was.
pub fn add_favorite(
    connection: &Connection,
    favorite: Favorite,
    now_ms: i64,
) -> rusqlite::Result<()> {
    let (column, id) = favorite.column();
    connection.execute(
        &format!("INSERT OR IGNORE INTO favorites ({column}, added_ms) VALUES (?1, ?2)"),
        params![id, now_ms],
    )?;
    Ok(())
}

/// Stops `favorite` being a favorite.
pub fn remove_favorite(connection: &Connection, favorite: Favorite) -> rusqlite::Result<()> {
    let (column, id) = favorite.column();
    connection.execute(&format!("DELETE FROM favorites WHERE {column} = ?1"), [id])?;
    Ok(())
}

/// Whether `favorite` is a favorite.
pub fn is_favorite(connection: &Connection, favorite: Favorite) -> rusqlite::Result<bool> {
    let (column, id) = favorite.column();
    connection.query_row(
        &format!("SELECT EXISTS (SELECT 1 FROM favorites WHERE {column} = ?1)"),
        [id],
        |row| row.get(0),
    )
}

/// Every favorite, in the order they were made.
pub fn favorites(connection: &Connection) -> rusqlite::Result<Vec<Favorite>> {
    let mut statement =
        connection.prepare("SELECT media_id, playlist_id FROM favorites ORDER BY added_ms, id")?;
    let rows = statement.query_map([], |row| {
        let media: Option<i64> = row.get(0)?;
        let playlist: Option<i64> = row.get(1)?;
        Ok(media
            .map(|id| Favorite::Media(MediaId(id)))
            .or(playlist.map(|id| Favorite::Playlist(PlaylistId(id)))))
    })?;
    rows.filter_map(Result::transpose).collect()
}

fn frequent(connection: &Connection, column: &str, limit: usize) -> rusqlite::Result<Vec<i64>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {column} FROM visits WHERE {column} IS NOT NULL
         ORDER BY count DESC, last_visited_ms DESC LIMIT ?1"
    ))?;
    let rows = statement.query_map([limit as i64], |row| row.get(0))?;
    rows.collect()
}
