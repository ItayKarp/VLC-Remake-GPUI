//! What the library knows about each media beyond its history (S4.1): its kind, tags, codecs and
//! resolution, in the `media` table's library columns. Metadata extraction (S4.3) writes them;
//! the views, sorting and search read them.

use std::path::PathBuf;
use std::time::Duration;

use rusqlite::{Connection, Row, params};

use super::artwork::ArtworkId;
use super::folders;
use super::media::MediaId;
use super::text_enum::text_enum;
use crate::settings::stored_path;

/// Whether a media is a video or audio, which decides its library section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MediaKind {
    /// Not probed yet.
    #[default]
    Unknown,
    /// It has a video track.
    Video,
    /// It has audio only.
    Audio,
}

text_enum!(MediaKind { Unknown => "unknown", Video => "video", Audio => "audio" });

/// A video's frame size in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// What metadata extraction found in a media.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    /// Video or audio.
    pub kind: MediaKind,
    /// The title tag (the views fall back to the file name).
    pub title: Option<String>,
    /// The artist tag.
    pub artist: Option<String>,
    /// The album tag.
    pub album: Option<String>,
    /// The album artist tag, which groups compilations under one album.
    pub album_artist: Option<String>,
    /// The track number on its disc.
    pub track_number: Option<u32>,
    /// The disc number in its album.
    pub disc_number: Option<u32>,
    /// The genre tag.
    pub genre: Option<String>,
    /// The year tag.
    pub year: Option<i32>,
    /// The codecs, for display: `H.264 / AAC`.
    pub codecs: Option<String>,
    /// The video's frame size.
    pub resolution: Option<Resolution>,
}

/// A media the library has a file of, whose metadata hasn't been read yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unprobed {
    /// The media.
    pub id: MediaId,
    /// One of its files, to read it from.
    pub path: PathBuf,
}

/// What reading a media found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extracted {
    /// Its tags, kind, codecs and resolution.
    pub metadata: Metadata,
    /// Its length, if the container says.
    pub duration: Option<Duration>,
    /// Its cover image, if it has one.
    pub artwork: Option<ArtworkId>,
}

/// Up to `limit` media after `after` (by row) that have a file on the disk (not online-only) and
/// whose metadata wasn't read yet: metadata extraction's next batch.
pub fn unprobed(
    connection: &Connection,
    after: MediaId,
    limit: usize,
) -> rusqlite::Result<Vec<Unprobed>> {
    // A bare column next to `MIN` comes from the row holding the minimum: the media's first file.
    let mut statement = connection.prepare(
        "SELECT media.id, folders.path, files.name, MIN(files.id)
         FROM media
         JOIN files ON files.media_id = media.id AND files.cloud = 0
         JOIN folders ON folders.id = files.folder_id
         WHERE media.probed_ms IS NULL AND media.id > ?1
         GROUP BY media.id ORDER BY media.id LIMIT ?2",
    )?;
    let rows = statement.query_map(params![after.0, limit as i64], |row| {
        let (folder, name): (String, String) = (row.get(1)?, row.get(2)?);
        Ok((MediaId(row.get(0)?), folders::decode(&folder).zip(stored_path::decode(&name))))
    })?;
    let rows: Vec<_> = rows.collect::<Result<_, _>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(id, path)| {
            path.map(|(folder, name)| Unprobed { id, path: folder.join(name) })
        })
        .collect())
}

/// Writes what reading media `id` found, at `now_ms`. A length the player already learned by
/// playing it is kept: playing measures it better than a header says.
pub fn set_extracted(
    connection: &Connection,
    id: MediaId,
    extracted: &Extracted,
    now_ms: i64,
) -> rusqlite::Result<()> {
    set(connection, id, &extracted.metadata)?;
    let duration = extracted.duration.map(|duration| duration.as_millis() as i64);
    connection.execute(
        "UPDATE media SET duration_ms = COALESCE(duration_ms, ?2), artwork_id = ?3, probed_ms = ?4
         WHERE id = ?1",
        params![id.0, duration, extracted.artwork.map(|artwork| artwork.0), now_ms],
    )?;
    Ok(())
}

/// Marks media `id` as read at `now_ms` without changing what is known about it (it couldn't be
/// parsed), so it isn't tried on every pass.
pub fn set_probed(connection: &Connection, id: MediaId, now_ms: i64) -> rusqlite::Result<()> {
    connection.execute("UPDATE media SET probed_ms = ?2 WHERE id = ?1", params![id.0, now_ms])?;
    Ok(())
}

/// Asks for media `id`'s metadata to be read again (its file was identified again), and its
/// thumbnail to be tried again if the thumbnailer gave up on it (S4.4).
pub fn reprobe(connection: &Connection, id: MediaId) -> rusqlite::Result<()> {
    connection.execute(
        "UPDATE media SET probed_ms = NULL, no_thumbnail_ms = NULL WHERE id = ?1",
        [id.0],
    )?;
    Ok(())
}

/// Writes `metadata` as what is known about media `id`.
pub fn set(connection: &Connection, id: MediaId, metadata: &Metadata) -> rusqlite::Result<()> {
    let resolution = metadata.resolution;
    connection.execute(
        "UPDATE media SET kind = ?2, title = ?3, artist = ?4, album = ?5, album_artist = ?6,
             track_number = ?7, disc_number = ?8, genre = ?9, year = ?10, codecs = ?11,
             width = ?12, height = ?13
         WHERE id = ?1",
        params![
            id.0,
            metadata.kind,
            metadata.title,
            metadata.artist,
            metadata.album,
            metadata.album_artist,
            metadata.track_number,
            metadata.disc_number,
            metadata.genre,
            metadata.year,
            metadata.codecs,
            resolution.map(|r| r.width),
            resolution.map(|r| r.height)
        ],
    )?;
    Ok(())
}

/// What is known about media `id`.
pub fn get(connection: &Connection, id: MediaId) -> rusqlite::Result<Metadata> {
    connection.query_row(
        "SELECT kind, title, artist, album, album_artist, track_number, disc_number, genre, year,
             codecs, width, height
         FROM media WHERE id = ?1",
        [id.0],
        read,
    )
}

fn read(row: &Row<'_>) -> rusqlite::Result<Metadata> {
    let (width, height): (Option<u32>, Option<u32>) = (row.get(10)?, row.get(11)?);
    Ok(Metadata {
        kind: row.get(0)?,
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        album_artist: row.get(4)?,
        track_number: row.get(5)?,
        disc_number: row.get(6)?,
        genre: row.get(7)?,
        year: row.get(8)?,
        codecs: row.get(9)?,
        resolution: width.zip(height).map(|(width, height)| Resolution { width, height }),
    })
}
