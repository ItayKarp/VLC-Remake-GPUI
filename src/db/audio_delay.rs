//! The `audio_delay` table (S7.6.3): the delay set for an audio track of a media, so it comes
//! back the next time the media plays, as the subtitle sync does (S6.3.4). A row is keyed by the
//! media's stored location and the track's stream id, and only delays that aren't zero are kept:
//! setting one back to zero deletes its row.
//!
//! Clearing the history forgets them too.

use rusqlite::{Connection, params};

use super::now_ms;

/// Version 11 (S7.6.3): each audio track's delay, by media and track.
pub(super) const MIGRATION: &str = "
    CREATE TABLE audio_delay (
        media TEXT NOT NULL,
        track INTEGER NOT NULL,
        delay_ms INTEGER NOT NULL,
        changed_ms INTEGER NOT NULL,
        PRIMARY KEY (media, track)
    );
";

/// The delays remembered for `media`'s audio tracks, in milliseconds by stream id.
pub fn load(connection: &Connection, media: &str) -> rusqlite::Result<Vec<(i32, i64)>> {
    let mut statement =
        connection.prepare_cached("SELECT track, delay_ms FROM audio_delay WHERE media = ?1")?;
    let rows = statement.query_map([media], |row| Ok((row.get(0)?, row.get(1)?)))?;
    rows.collect()
}

/// Remembers `delay_ms` for audio `track` of `media`; zero forgets it.
pub fn save(
    connection: &Connection,
    media: &str,
    track: i32,
    delay_ms: i64,
) -> rusqlite::Result<()> {
    if delay_ms == 0 {
        connection.execute(
            "DELETE FROM audio_delay WHERE media = ?1 AND track = ?2",
            params![media, track],
        )?;
        return Ok(());
    }
    connection.execute(
        "INSERT INTO audio_delay (media, track, delay_ms, changed_ms) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (media, track) DO UPDATE SET delay_ms = excluded.delay_ms,
             changed_ms = excluded.changed_ms",
        params![media, track, delay_ms, now_ms()],
    )?;
    Ok(())
}

/// Forgets every remembered delay (Clear History).
pub fn clear(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM audio_delay", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn a_delay_is_saved_replaced_and_forgotten() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        save(connection, "/m.mkv", 1, -300).unwrap();
        save(connection, "/m.mkv", 2, 150).unwrap();
        save(connection, "/other.mkv", 1, 50).unwrap();
        save(connection, "/m.mkv", 1, 450).unwrap();
        save(connection, "/m.mkv", 2, 0).unwrap();
        assert_eq!(load(connection, "/m.mkv").unwrap(), [(1, 450)]);

        clear(connection).unwrap();
        assert_eq!(load(connection, "/other.mkv").unwrap(), []);
    }
}
