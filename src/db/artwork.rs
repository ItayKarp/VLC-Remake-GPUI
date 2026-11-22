//! The `artwork` table (S4.1, S4.3): each cover image the library keeps in its cache, by the
//! cache key (a hash of the image), and where it came from. Media point at one with
//! `media.artwork_id`, so an album's tracks share one row and one cached image.

use rusqlite::{Connection, OptionalExtension as _, params};

use super::media::MediaId;
use super::text_enum::text_enum;

/// A row of the `artwork` table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ArtworkId(pub i64);

/// Where a cover image came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtworkSource {
    /// Inside the media file (an ID3 picture, an MP4 cover, a Matroska attachment).
    Embedded,
    /// An image next to the file (`cover.jpg`, `folder.jpg`, `AlbumArt*.jpg`).
    Folder,
    /// Fetched from the internet, which only happens when the user allows it (S4.3.4).
    Online,
}

text_enum!(ArtworkSource { Embedded => "embedded", Folder => "folder", Online => "online" });

/// A cached cover image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artwork {
    /// The row.
    pub id: ArtworkId,
    /// The image's key in the artwork cache.
    pub cache_key: String,
    /// Where it came from.
    pub source: ArtworkSource,
}

/// The row of the image cached as `cache_key`, added if it is new. An image found again from
/// another source keeps the source it was first found with.
pub fn ensure(
    connection: &Connection,
    cache_key: &str,
    source: ArtworkSource,
) -> rusqlite::Result<ArtworkId> {
    connection.query_row(
        "INSERT INTO artwork (cache_key, source) VALUES (?1, ?2)
         ON CONFLICT (cache_key) DO UPDATE SET cache_key = excluded.cache_key
         RETURNING id",
        params![cache_key, source],
        |row| row.get(0).map(ArtworkId),
    )
}

/// The artwork of media `id`, if it has some.
pub fn of_media(connection: &Connection, id: MediaId) -> rusqlite::Result<Option<Artwork>> {
    connection
        .query_row(
            "SELECT artwork.id, artwork.cache_key, artwork.source
             FROM media JOIN artwork ON artwork.id = media.artwork_id WHERE media.id = ?1",
            [id.0],
            |row| {
                Ok(Artwork {
                    id: ArtworkId(row.get(0)?),
                    cache_key: row.get(1)?,
                    source: row.get(2)?,
                })
            },
        )
        .optional()
}
