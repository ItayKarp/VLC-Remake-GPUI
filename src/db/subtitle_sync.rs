//! The `subtitle_sync` table (S6.3.4): the delay, speed and duration factor set for a subtitle
//! track of a media, so they come back the next time it plays. A row is keyed by the media's
//! stored location and the subtitle's own key (its file's location, or its stream), and only
//! syncs that change something are kept: setting one back to its default deletes its row.
//!
//! Clearing the history forgets them too.

use rusqlite::{Connection, params};

use super::now_ms;

/// Version 9 (S6.3.4): each subtitle track's sync, by media and subtitle.
pub(super) const MIGRATION: &str = "
    CREATE TABLE subtitle_sync (
        media TEXT NOT NULL,
        subtitle TEXT NOT NULL,
        delay_ms INTEGER NOT NULL,
        speed REAL NOT NULL,
        duration_factor REAL NOT NULL,
        changed_ms INTEGER NOT NULL,
        PRIMARY KEY (media, subtitle)
    );
";

/// A subtitle track's remembered sync, as stored.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SavedSync {
    /// The delay in milliseconds; positive shows the subtitles later.
    pub delay_ms: i64,
    /// How fast the subtitle file's clock runs against the video's.
    pub speed: f64,
    /// How much longer each line stays up; 0 is off.
    pub duration_factor: f64,
}

/// The syncs remembered for `media`'s subtitles, by subtitle key.
pub fn load(connection: &Connection, media: &str) -> rusqlite::Result<Vec<(String, SavedSync)>> {
    let mut statement = connection.prepare_cached(
        "SELECT subtitle, delay_ms, speed, duration_factor FROM subtitle_sync WHERE media = ?1",
    )?;
    let rows = statement.query_map([media], |row| {
        let sync =
            SavedSync { delay_ms: row.get(1)?, speed: row.get(2)?, duration_factor: row.get(3)? };
        Ok((row.get(0)?, sync))
    })?;
    rows.collect()
}

/// Remembers `sync` for `subtitle` of `media`, or forgets it with `None`.
pub fn save(
    connection: &Connection,
    media: &str,
    subtitle: &str,
    sync: Option<SavedSync>,
) -> rusqlite::Result<()> {
    let Some(sync) = sync else {
        connection.execute(
            "DELETE FROM subtitle_sync WHERE media = ?1 AND subtitle = ?2",
            params![media, subtitle],
        )?;
        return Ok(());
    };
    connection.execute(
        "INSERT INTO subtitle_sync (media, subtitle, delay_ms, speed, duration_factor, changed_ms)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (media, subtitle) DO UPDATE SET delay_ms = excluded.delay_ms,
             speed = excluded.speed, duration_factor = excluded.duration_factor,
             changed_ms = excluded.changed_ms",
        params![media, subtitle, sync.delay_ms, sync.speed, sync.duration_factor, now_ms()],
    )?;
    Ok(())
}

/// Forgets every remembered sync (Clear History).
pub fn clear(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM subtitle_sync", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn a_sync_is_saved_replaced_and_forgotten() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        let sync = SavedSync { delay_ms: -4_000, speed: 1.043, duration_factor: 0. };
        save(connection, "/m.mkv", "file:/m.srt", Some(sync)).unwrap();
        save(connection, "/m.mkv", "stream:3", Some(sync)).unwrap();
        save(connection, "/other.mkv", "stream:3", Some(sync)).unwrap();
        let later = SavedSync { delay_ms: 250, ..sync };
        save(connection, "/m.mkv", "file:/m.srt", Some(later)).unwrap();
        save(connection, "/m.mkv", "stream:3", None).unwrap();
        assert_eq!(load(connection, "/m.mkv").unwrap(), [("file:/m.srt".to_owned(), later)]);

        clear(connection).unwrap();
        assert_eq!(load(connection, "/other.mkv").unwrap(), []);
    }
}
