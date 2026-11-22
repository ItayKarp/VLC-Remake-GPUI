//! What the user did in the library (S4.1), for Home (S4.9): plays (Frequently Listened
//! Artists), visits to folders and playlists (Frequently Visited), and favorites. Listen Again
//! and Continue Watching read the media rows ([`played`](super::played)).
//!
//! A play is written by [`media::record_play`](super::media::record_play); Clear History
//! forgets plays and visits, but not favorites.

use rusqlite::{Connection, OptionalExtension as _, params};

use super::folders::FolderId;
use super::media::MediaId;
use super::playlists::PlaylistId;
use crate::library::frequency::{self, Scored};

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

/// Counts a visit to `place` at `now_ms`, raising its fading score
/// ([`frequency`](crate::library::frequency)).
pub fn record_visit(connection: &Connection, place: Place, now_ms: i64) -> rusqlite::Result<()> {
    let (column, id) = place.column();
    let previous = connection
        .query_row(
            &format!("SELECT score, last_visited_ms FROM visits WHERE {column} = ?1"),
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let score = frequency::raised(previous, now_ms);
    connection.execute(
        &format!(
            "INSERT INTO visits ({column}, count, last_visited_ms, score) VALUES (?1, 1, ?2, ?3)
             ON CONFLICT ({column}) DO UPDATE SET count = count + 1, last_visited_ms = ?2,
                 score = ?3"
        ),
        params![id, now_ms, score],
    )?;
    Ok(())
}

/// The `limit` folders visited most as of `now_ms`, recent visits counting more.
pub fn frequent_folders(
    connection: &Connection,
    now_ms: i64,
    limit: usize,
) -> rusqlite::Result<Vec<FolderId>> {
    let ranked = frequent(connection, "folder_id", now_ms, limit)?;
    Ok(ranked.into_iter().map(FolderId).collect())
}

/// The `limit` playlists visited most as of `now_ms`, recent visits counting more.
pub fn frequent_playlists(
    connection: &Connection,
    now_ms: i64,
    limit: usize,
) -> rusqlite::Result<Vec<PlaylistId>> {
    let ranked = frequent(connection, "playlist_id", now_ms, limit)?;
    Ok(ranked.into_iter().map(PlaylistId).collect())
}

/// The `limit` artists played most as of `now_ms` (Frequently Listened Artists), by album
/// artist when tagged, recent plays counting more.
pub fn frequent_artists(
    connection: &Connection,
    now_ms: i64,
    limit: usize,
) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT COALESCE(media.album_artist, media.artist) AS name, plays.played_ms FROM plays
         JOIN media ON media.id = plays.media_id
         WHERE media.kind = 'audio' AND name IS NOT NULL",
    )?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let uses: Vec<(String, i64)> = rows.collect::<Result<_, _>>()?;
    Ok(frequency::top(frequency::sum_uses(uses, now_ms), limit))
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

/// Whether `favorite` is a favorite. The app asks the Favorites global; tests ask the table.
#[cfg(test)]
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

fn frequent(
    connection: &Connection,
    column: &str,
    now_ms: i64,
    limit: usize,
) -> rusqlite::Result<Vec<i64>> {
    let mut statement = connection.prepare(&format!(
        "SELECT {column}, score, last_visited_ms FROM visits WHERE {column} IS NOT NULL"
    ))?;
    let rows = statement.query_map([], |row| {
        let (score, last_ms) = (row.get(1)?, row.get(2)?);
        let score = frequency::decayed(score, now_ms - last_ms);
        Ok(Scored { item: row.get(0)?, score, last_ms })
    })?;
    Ok(frequency::top(rows.collect::<Result<_, _>>()?, limit))
}
