//! The `thumbnails` table (S4.1, S4.4): which cached images show each media at each size, and
//! what the cache needs to evict the least recently shown ones. A media the thumbnailer gave up
//! on is marked in `media.no_thumbnail_ms`, so it isn't tried again on every launch.

use std::path::PathBuf;
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension as _, params};

use super::folders;
use super::media::MediaId;
use super::metadata::MediaKind;
use super::text_enum::text_enum;
use crate::settings::stored_path;

/// The sizes the library shows thumbnails at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SizeClass {
    /// A list row's square tile.
    List,
    /// A grid tile.
    Grid,
}

text_enum!(SizeClass { List => "list", Grid => "grid" });

/// One size of a thumbnail: a size class at a display scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Variant {
    /// The tile it fills.
    pub class: SizeClass,
    /// 1, or 2 for HiDPI displays.
    pub scale: u8,
}

/// Every variant, made together from one picture.
pub const VARIANTS: [Variant; 4] = [
    Variant { class: SizeClass::List, scale: 1 },
    Variant { class: SizeClass::List, scale: 2 },
    Variant { class: SizeClass::Grid, scale: 1 },
    Variant { class: SizeClass::Grid, scale: 2 },
];

/// One cached image of a media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cached {
    /// The size it is.
    pub variant: Variant,
    /// The image's key in the thumbnail cache.
    pub cache_key: String,
    /// How large its file is.
    pub bytes: u64,
}

/// What the thumbnailer needs to know about a media.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subject {
    /// The media.
    pub id: MediaId,
    /// Its content identity, which names its pictures in the cache.
    pub identity: String,
    /// Video, audio, or not read yet.
    pub kind: MediaKind,
    /// Its length, if known.
    pub duration: Option<Duration>,
    /// One of its files on the disk (not online-only), if the library has one.
    pub path: Option<PathBuf>,
    /// Its cover's key in the artwork cache.
    pub artwork: Option<String>,
    /// Whether the thumbnailer gave up on it.
    pub gave_up: bool,
}

/// What the thumbnailer needs to know about media `id`, if it exists.
pub fn subject(connection: &Connection, id: MediaId) -> rusqlite::Result<Option<Subject>> {
    let found = connection
        .query_row(
            "SELECT media.identity, media.kind, media.duration_ms, artwork.cache_key,
                 media.no_thumbnail_ms IS NOT NULL
             FROM media LEFT JOIN artwork ON artwork.id = media.artwork_id WHERE media.id = ?1",
            [id.0],
            |row| {
                let duration: Option<i64> = row.get(2)?;
                Ok(Subject {
                    id,
                    identity: row.get(0)?,
                    kind: row.get(1)?,
                    duration: duration
                        .and_then(|ms| u64::try_from(ms).ok())
                        .map(Duration::from_millis),
                    path: None,
                    artwork: row.get(3)?,
                    gave_up: row.get(4)?,
                })
            },
        )
        .optional()?;
    let Some(mut subject) = found else { return Ok(None) };
    subject.path = local_file(connection, id)?;
    Ok(Some(subject))
}

/// The first file of media `id` that is on the disk.
fn local_file(connection: &Connection, id: MediaId) -> rusqlite::Result<Option<PathBuf>> {
    let stored: Option<(String, String)> = connection
        .query_row(
            "SELECT folders.path, files.name FROM files
             JOIN folders ON folders.id = files.folder_id
             WHERE files.media_id = ?1 AND files.cloud = 0 ORDER BY files.id LIMIT 1",
            [id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(stored.and_then(|(folder, name)| {
        let (folder, name) = (folders::decode(&folder)?, stored_path::decode(&name)?);
        Some(folder.join(name))
    }))
}

/// The cached images of media `id`.
pub fn cached(connection: &Connection, id: MediaId) -> rusqlite::Result<Vec<Cached>> {
    let mut statement = connection.prepare(
        "SELECT size_class, scale, cache_key, bytes FROM thumbnails WHERE media_id = ?1",
    )?;
    let rows = statement.query_map([id.0], |row| {
        Ok(Cached {
            variant: Variant { class: row.get(0)?, scale: row.get(1)? },
            cache_key: row.get(2)?,
            bytes: row.get::<_, i64>(3)?.unsigned_abs(),
        })
    })?;
    rows.collect()
}

/// Records `images` as media `id`'s thumbnails, shown at `now_ms`, replacing what it had, and
/// clears its "gave up" mark.
pub fn record(
    connection: &mut Connection,
    id: MediaId,
    images: &[Cached],
    now_ms: i64,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute("DELETE FROM thumbnails WHERE media_id = ?1", [id.0])?;
    for image in images {
        transaction.execute(
            "INSERT INTO thumbnails (media_id, size_class, scale, cache_key, created_ms, bytes,
                 used_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?5)",
            params![
                id.0,
                image.variant.class,
                image.variant.scale,
                image.cache_key,
                now_ms,
                i64::try_from(image.bytes).unwrap_or(i64::MAX)
            ],
        )?;
    }
    transaction.execute("UPDATE media SET no_thumbnail_ms = NULL WHERE id = ?1", [id.0])?;
    transaction.commit()
}

/// Marks media `id` as one the thumbnailer gave up on at `now_ms`. Set before each try, so a file
/// that crashes the app isn't tried again either.
pub fn give_up(connection: &Connection, id: MediaId, now_ms: i64) -> rusqlite::Result<()> {
    connection
        .execute("UPDATE media SET no_thumbnail_ms = ?2 WHERE id = ?1", params![id.0, now_ms])?;
    Ok(())
}

/// Clears media `id`'s "gave up" mark: the try was cut short, not failed.
pub fn try_again(connection: &Connection, id: MediaId) -> rusqlite::Result<()> {
    connection.execute("UPDATE media SET no_thumbnail_ms = NULL WHERE id = ?1", [id.0])?;
    Ok(())
}

/// Forgets media `id`'s thumbnails (their files are gone).
pub fn forget(connection: &Connection, id: MediaId) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM thumbnails WHERE media_id = ?1", [id.0])?;
    Ok(())
}

/// Notes that media `id`'s thumbnails were shown at `now_ms`, for eviction.
pub fn touch(connection: &Connection, id: MediaId, now_ms: i64) -> rusqlite::Result<()> {
    connection
        .execute("UPDATE thumbnails SET used_ms = ?2 WHERE media_id = ?1", params![id.0, now_ms])?;
    Ok(())
}

/// Up to `limit` videos that were read, have a file on the disk, and have neither thumbnails nor
/// a "gave up" mark: the background thumbnailer's next batch, newest first.
pub fn backlog(connection: &Connection, limit: usize) -> rusqlite::Result<Vec<MediaId>> {
    let mut statement = connection.prepare(
        "SELECT media.id FROM media
         WHERE media.kind = 'video' AND media.probed_ms IS NOT NULL
             AND media.no_thumbnail_ms IS NULL
             AND NOT EXISTS (SELECT 1 FROM thumbnails WHERE thumbnails.media_id = media.id)
             AND EXISTS (SELECT 1 FROM files WHERE files.media_id = media.id AND files.cloud = 0)
         ORDER BY media.added_ms DESC, media.id DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit as i64], |row| row.get(0).map(MediaId))?;
    rows.collect()
}

/// How large the cache is: each cached image counted once, however many media show it.
pub fn total_bytes(connection: &Connection) -> rusqlite::Result<u64> {
    connection.query_row(
        "SELECT COALESCE(SUM(bytes), 0) FROM
             (SELECT MAX(bytes) AS bytes FROM thumbnails GROUP BY cache_key)",
        [],
        |row| row.get::<_, i64>(0).map(i64::unsigned_abs),
    )
}

/// Up to `limit` cached images, least recently shown first, with their sizes.
pub fn least_recent(connection: &Connection, limit: usize) -> rusqlite::Result<Vec<(String, u64)>> {
    let mut statement = connection.prepare(
        "SELECT cache_key, MAX(bytes) FROM thumbnails GROUP BY cache_key
         ORDER BY MAX(used_ms), cache_key LIMIT ?1",
    )?;
    let rows = statement
        .query_map([limit as i64], |row| Ok((row.get(0)?, row.get::<_, i64>(1)?.unsigned_abs())))?;
    rows.collect()
}

/// Forgets the image cached as `cache_key`, for every media it showed.
pub fn evict(connection: &Connection, cache_key: &str) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM thumbnails WHERE cache_key = ?1", [cache_key])?;
    Ok(())
}

#[cfg(test)]
mod tests;
