//! What the library knows about each media beyond its history (S4.1): its kind, tags, codecs and
//! resolution, in the `media` table's library columns. Metadata extraction (S4.3) writes them;
//! the views, sorting and search read them.

use rusqlite::{Connection, Row, params};

use super::media::MediaId;
use super::text_enum::text_enum;

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
