//! The `scrobbles` table (S8.17.1): played tracks waiting to be submitted to last.fm. A track is
//! queued once it counts as played and deleted once last.fm took it, so a submission that fails
//! (offline, the service down) leaves it for the next one. The queue is capped: past
//! [`MAX_QUEUED`] the oldest are dropped, as are those older than last.fm still accepts.

use rusqlite::{Connection, params};

/// The most tracks kept waiting; queuing more drops the oldest.
pub const MAX_QUEUED: usize = 1000;
/// How old a play last.fm still takes, in seconds: two weeks.
pub const MAX_AGE_SECONDS: i64 = 14 * 24 * 60 * 60;

/// Version 13 (S8.17.1): the scrobble queue.
pub(super) const MIGRATION: &str = "
    CREATE TABLE scrobbles (
        id INTEGER PRIMARY KEY,
        artist TEXT NOT NULL,
        title TEXT NOT NULL,
        album TEXT,
        duration_s INTEGER,
        -- When the track started playing, in seconds since the Unix epoch.
        started_s INTEGER NOT NULL
    );
";

/// One played track.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Play {
    /// The artist tag.
    pub artist: String,
    /// The title tag.
    pub title: String,
    /// The album tag, if any.
    pub album: Option<String>,
    /// Its length in seconds, when known.
    pub duration_s: Option<u32>,
    /// When it started playing, in seconds since the Unix epoch.
    pub started_s: i64,
}

/// A queued play and its row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Queued {
    /// The row, to delete once submitted.
    pub id: i64,
    /// The play.
    pub play: Play,
}

/// Queues `play`, dropping the oldest beyond [`MAX_QUEUED`].
pub fn enqueue(connection: &mut Connection, play: &Play) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO scrobbles (artist, title, album, duration_s, started_s)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![play.artist, play.title, play.album, play.duration_s, play.started_s],
    )?;
    transaction.execute(
        "DELETE FROM scrobbles WHERE id NOT IN (
             SELECT id FROM scrobbles ORDER BY started_s DESC, id DESC LIMIT ?1)",
        [MAX_QUEUED as i64],
    )?;
    transaction.commit()
}

/// The oldest `limit` plays waiting, oldest first, after dropping those older than last.fm
/// takes at `now_s`.
pub fn next_batch(
    connection: &Connection,
    now_s: i64,
    limit: usize,
) -> rusqlite::Result<Vec<Queued>> {
    connection.execute("DELETE FROM scrobbles WHERE started_s < ?1", [now_s - MAX_AGE_SECONDS])?;
    let mut statement = connection.prepare(
        "SELECT id, artist, title, album, duration_s, started_s FROM scrobbles
         ORDER BY started_s, id LIMIT ?1",
    )?;
    let rows = statement.query_map([limit as i64], |row| {
        Ok(Queued {
            id: row.get(0)?,
            play: Play {
                artist: row.get(1)?,
                title: row.get(2)?,
                album: row.get(3)?,
                duration_s: row.get(4)?,
                started_s: row.get(5)?,
            },
        })
    });
    rows?.collect()
}

/// Deletes the plays `ids`, submitted or refused for good.
pub fn remove(connection: &mut Connection, ids: &[i64]) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    for id in ids {
        transaction.execute("DELETE FROM scrobbles WHERE id = ?1", [id])?;
    }
    transaction.commit()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    const NOW: i64 = 1_800_000_000;

    fn play(title: &str, started_s: i64) -> Play {
        Play {
            artist: "Artist".into(),
            title: title.into(),
            album: None,
            duration_s: Some(200),
            started_s,
        }
    }

    fn count(connection: &Connection) -> rusqlite::Result<usize> {
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM scrobbles", [], |row| row.get(0))?;
        Ok(count as usize)
    }

    fn titles(batch: &[Queued]) -> Vec<&str> {
        batch.iter().map(|queued| queued.play.title.as_str()).collect()
    }

    #[test]
    fn plays_wait_oldest_first_until_removed() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        enqueue(connection, &play("b", NOW - 10)).expect("enqueue");
        enqueue(connection, &play("a", NOW - 20)).expect("enqueue");
        let batch = next_batch(connection, NOW, 50).expect("batch");
        assert_eq!(titles(&batch), ["a", "b"]);
        assert_eq!(batch[0].play, play("a", NOW - 20));
        remove(connection, &[batch[0].id]).expect("remove");
        assert_eq!(titles(&next_batch(connection, NOW, 50).expect("batch")), ["b"]);
    }

    #[test]
    fn a_batch_is_limited_and_plays_too_old_are_dropped() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        enqueue(connection, &play("old", NOW - MAX_AGE_SECONDS - 1)).expect("enqueue");
        for n in 0..3 {
            enqueue(connection, &play(&n.to_string(), NOW - 100 + n)).expect("enqueue");
        }
        assert_eq!(titles(&next_batch(connection, NOW, 2).expect("batch")), ["0", "1"]);
        assert_eq!(count(connection).expect("count"), 3);
    }

    #[test]
    fn the_queue_keeps_the_latest_plays() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        for n in 0..=MAX_QUEUED as i64 {
            enqueue(connection, &play(&n.to_string(), NOW - 5000 + n)).expect("enqueue");
        }
        assert_eq!(count(connection).expect("count"), MAX_QUEUED);
        assert_eq!(titles(&next_batch(connection, NOW, 1).expect("batch")), ["1"]);
    }
}
