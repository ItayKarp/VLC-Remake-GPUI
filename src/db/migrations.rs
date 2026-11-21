//! The database schema, as a list of migrations. `PRAGMA user_version` holds how many have run;
//! each one runs in its own transaction, so a crash leaves the database at the previous version.
//!
//! Never edit a migration that has shipped: add a new one at the end.

use rusqlite::Connection;

use super::Unusable;

/// Each migration's SQL, oldest first. The schema version is how many have run.
pub(super) const MIGRATIONS: &[&str] = &[MEDIA, PLAYLISTS];

/// Version 1 (S1.7): what the player remembers about each media, by identity (S1.7.2).
const MEDIA: &str = "
    CREATE TABLE media (
        id INTEGER PRIMARY KEY,
        -- The content key of a file (xxh3 of its size, head and tail) or `url:` and a URL.
        identity TEXT NOT NULL,
        -- Where it was last seen: a stored path (`settings::stored_path`) or the URL.
        location TEXT NOT NULL,
        size INTEGER,
        modified_ms INTEGER,
        duration_ms INTEGER,
        -- Where to resume; NULL to start from the beginning.
        position_ms INTEGER,
        watched INTEGER NOT NULL DEFAULT 0,
        play_count INTEGER NOT NULL DEFAULT 0,
        last_played_ms INTEGER,
        -- When the file at `location` changed into other media; purged 30 days later.
        orphaned_ms INTEGER
    );
    CREATE INDEX media_identity ON media (identity);
    CREATE INDEX media_location ON media (location);
";

/// Version 2 (S3.5): saved playlists and their items, in order. Stage 4 (S4.1) adds the rest of
/// the library around them.
const PLAYLISTS: &str = "
    CREATE TABLE playlists (
        id INTEGER PRIMARY KEY,
        title TEXT NOT NULL,
        -- A custom thumbnail image as a stored path; NULL for the automatic collage (S3.5.6).
        thumbnail TEXT,
        created_ms INTEGER NOT NULL,
        modified_ms INTEGER NOT NULL
    );
    CREATE TABLE playlist_items (
        playlist_id INTEGER NOT NULL,
        position INTEGER NOT NULL,
        -- A stored path or a URL, as `media.location`.
        location TEXT NOT NULL,
        PRIMARY KEY (playlist_id, position)
    );
";

/// The schema version this build writes.
pub const CURRENT_VERSION: u32 = MIGRATIONS.len() as u32;

/// Runs the migrations `connection` hasn't had yet. A database from a newer build is refused
/// rather than touched.
pub(super) fn run(connection: &mut Connection) -> Result<(), Unusable> {
    let version = version(connection)?;
    if version > CURRENT_VERSION {
        return Err(Unusable::Unavailable(format!(
            "schema version {version} is newer than this build's {CURRENT_VERSION}"
        )));
    }
    for (done, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        // A migration that fails for any reason but a busy, read-only or full disk means the
        // schema isn't what this build expects.
        let failed = |error: rusqlite::Error| {
            let why = format!("migration {} failed: {error}", done + 1);
            if super::is_transient(&error) {
                Unusable::Unavailable(why)
            } else {
                Unusable::Corrupt(why)
            }
        };
        let transaction = connection.transaction().map_err(failed)?;
        transaction.execute_batch(sql).map_err(failed)?;
        transaction.pragma_update(None, "user_version", done as u32 + 1).map_err(failed)?;
        transaction.commit().map_err(failed)?;
    }
    Ok(())
}

/// The schema version `connection` is at.
pub(super) fn version(connection: &Connection) -> Result<u32, Unusable> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| super::classify(&error))
}
