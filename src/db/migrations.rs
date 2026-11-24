//! The database schema, as a list of migrations. `PRAGMA user_version` holds how many have run;
//! each one runs in its own transaction, so a crash leaves the database at the previous version.
//!
//! Never edit a migration that has shipped: add a new one at the end.

use rusqlite::Connection;

use super::Unusable;

/// Each migration's SQL, oldest first. The schema version is how many have run.
pub(super) const MIGRATIONS: &[&str] = &[
    MEDIA,
    PLAYLISTS,
    LIBRARY,
    INDEXING,
    EXTRACTION,
    THUMBNAILS,
    super::search::MIGRATION,
    HOME,
    super::subtitle_sync::MIGRATION,
    super::subtitle_links::MIGRATION,
    super::audio_delay::MIGRATION,
    super::bookmarks::MIGRATION,
    super::scrobbles::MIGRATION,
    super::podcasts::MIGRATION,
];

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

/// Version 3 (S4.1): the library around the history and the playlists.
///
/// - A folder is one row however many roots reach it (nested roots share their folders), and a
///   file is one row per path. Files point at `media` by identity, so the same file reached
///   through two roots or a symlink is one media row shown in both trees.
/// - `key` columns hold a path's comparison key (`library::path_key`), which folds case and
///   Unicode normalization the way the root's file system does; `path` and `name` keep the
///   original, as `settings::stored_path` writes it, for opening.
/// - Plays already counted in `media` become one `plays` row at their last play, so Listen Again
///   isn't empty after the upgrade.
const LIBRARY: &str = "
    CREATE TABLE folders (
        id INTEGER PRIMARY KEY,
        -- NULL for a root's top folder, or once the folder above was removed with its root.
        parent_id INTEGER REFERENCES folders (id) ON DELETE SET NULL,
        path TEXT NOT NULL,
        key TEXT NOT NULL UNIQUE
    );
    CREATE INDEX folders_parent ON folders (parent_id);
    CREATE TABLE roots (
        id INTEGER PRIMARY KEY,
        folder_id INTEGER NOT NULL UNIQUE REFERENCES folders (id) ON DELETE CASCADE,
        kind TEXT NOT NULL CHECK (kind IN ('video', 'music', 'mixed')),
        status TEXT NOT NULL DEFAULT 'ok'
            CHECK (status IN ('ok', 'missing', 'permission_denied', 'slow')),
        case_sensitive INTEGER NOT NULL,
        added_ms INTEGER NOT NULL,
        scanned_ms INTEGER
    );
    CREATE TABLE artwork (
        id INTEGER PRIMARY KEY,
        -- The image's key in the cache dir.
        cache_key TEXT NOT NULL UNIQUE,
        source TEXT NOT NULL CHECK (source IN ('embedded', 'folder', 'online'))
    );
    ALTER TABLE media ADD COLUMN kind TEXT NOT NULL DEFAULT 'unknown'
        CHECK (kind IN ('unknown', 'video', 'audio'));
    ALTER TABLE media ADD COLUMN title TEXT;
    ALTER TABLE media ADD COLUMN artist TEXT;
    ALTER TABLE media ADD COLUMN album TEXT;
    ALTER TABLE media ADD COLUMN album_artist TEXT;
    ALTER TABLE media ADD COLUMN track_number INTEGER;
    ALTER TABLE media ADD COLUMN disc_number INTEGER;
    ALTER TABLE media ADD COLUMN genre TEXT;
    ALTER TABLE media ADD COLUMN year INTEGER;
    ALTER TABLE media ADD COLUMN added_ms INTEGER NOT NULL DEFAULT 0;
    -- What the codecs are, for display: `H.264 / AAC`.
    ALTER TABLE media ADD COLUMN codecs TEXT;
    ALTER TABLE media ADD COLUMN width INTEGER;
    ALTER TABLE media ADD COLUMN height INTEGER;
    -- Has artwork when set.
    ALTER TABLE media ADD COLUMN artwork_id INTEGER REFERENCES artwork (id) ON DELETE SET NULL;
    UPDATE media SET added_ms = COALESCE(last_played_ms, 0);
    CREATE TABLE files (
        id INTEGER PRIMARY KEY,
        folder_id INTEGER NOT NULL REFERENCES folders (id) ON DELETE CASCADE,
        name TEXT NOT NULL,
        key TEXT NOT NULL,
        -- NULL until the file is identified, or after its media row was purged.
        media_id INTEGER REFERENCES media (id) ON DELETE SET NULL,
        size INTEGER NOT NULL,
        modified_ms INTEGER,
        UNIQUE (folder_id, key)
    );
    CREATE INDEX files_media ON files (media_id);
    CREATE TABLE thumbnails (
        media_id INTEGER NOT NULL REFERENCES media (id) ON DELETE CASCADE,
        size_class TEXT NOT NULL CHECK (size_class IN ('list', 'grid')),
        -- 1, or 2 for HiDPI.
        scale INTEGER NOT NULL,
        cache_key TEXT NOT NULL,
        created_ms INTEGER NOT NULL,
        PRIMARY KEY (media_id, size_class, scale)
    );
    CREATE TABLE favorites (
        id INTEGER PRIMARY KEY,
        media_id INTEGER UNIQUE REFERENCES media (id) ON DELETE CASCADE,
        playlist_id INTEGER UNIQUE REFERENCES playlists (id) ON DELETE CASCADE,
        added_ms INTEGER NOT NULL,
        CHECK ((media_id IS NULL) != (playlist_id IS NULL))
    );
    CREATE TABLE visits (
        id INTEGER PRIMARY KEY,
        folder_id INTEGER UNIQUE REFERENCES folders (id) ON DELETE CASCADE,
        playlist_id INTEGER UNIQUE REFERENCES playlists (id) ON DELETE CASCADE,
        count INTEGER NOT NULL,
        last_visited_ms INTEGER NOT NULL,
        CHECK ((folder_id IS NULL) != (playlist_id IS NULL))
    );
    CREATE TABLE plays (
        id INTEGER PRIMARY KEY,
        media_id INTEGER NOT NULL REFERENCES media (id) ON DELETE CASCADE,
        played_ms INTEGER NOT NULL
    );
    CREATE INDEX plays_media ON plays (media_id, played_ms);
    CREATE INDEX plays_time ON plays (played_ms);
    INSERT INTO plays (media_id, played_ms)
        SELECT id, last_played_ms FROM media WHERE play_count > 0 AND last_played_ms IS NOT NULL;
    CREATE TABLE network_streams (
        id INTEGER PRIMARY KEY,
        url TEXT NOT NULL,
        title TEXT,
        -- `recent`, `podcasts`, `radio`…: which categories exist is decided with S4.11.3.
        category TEXT NOT NULL,
        added_ms INTEGER NOT NULL,
        last_played_ms INTEGER,
        UNIQUE (category, url)
    );
";

/// Version 4 (S4.2): what the indexer learns besides the files themselves.
///
/// - `files.cloud` marks an online-only file (OneDrive Files On-Demand, iCloud Drive, Dropbox):
///   its content is never read during a scan, since that would download it, so it has no media
///   row until it is on the disk.
/// - `unreadable_folders` lists the folders a scan of a root couldn't read (permission denied),
///   for the root's status details. A scan replaces its root's list.
const INDEXING: &str = "
    ALTER TABLE files ADD COLUMN cloud INTEGER NOT NULL DEFAULT 0;
    CREATE INDEX files_unidentified ON files (id) WHERE media_id IS NULL;
    CREATE TABLE unreadable_folders (
        root_id INTEGER NOT NULL REFERENCES roots (id) ON DELETE CASCADE,
        path TEXT NOT NULL,
        PRIMARY KEY (root_id, path)
    );
";

/// Version 5 (S4.3): when each media's tags were last read.
///
/// `media.probed_ms` is NULL until metadata extraction has read the media (or tried to: a file
/// it can't parse is marked too, so it isn't read on every pass). Identifying a file again
/// clears it, so Resync reads the tags again.
const EXTRACTION: &str = "
    ALTER TABLE media ADD COLUMN probed_ms INTEGER;
    CREATE INDEX media_unprobed ON media (id) WHERE probed_ms IS NULL;
";

/// Version 6 (S4.4): what the thumbnail cache needs to evict, and what not to try again.
///
/// - `thumbnails.bytes` and `used_ms` size the cache and find its least recently shown images.
///   One cached image can serve several media (an album's tracks share a cover), so rows are
///   counted per `cache_key`.
/// - `media.no_thumbnail_ms` is set when the thumbnailer starts on a media and cleared when it
///   succeeds, so a file it gave up on, or that crashed the app, isn't tried on every launch.
///   Identifying the file again (Resync) clears it, like `probed_ms`.
const THUMBNAILS: &str = "
    ALTER TABLE thumbnails ADD COLUMN bytes INTEGER NOT NULL DEFAULT 0;
    ALTER TABLE thumbnails ADD COLUMN used_ms INTEGER NOT NULL DEFAULT 0;
    CREATE INDEX thumbnails_key ON thumbnails (cache_key);
    ALTER TABLE media ADD COLUMN no_thumbnail_ms INTEGER;
";

/// Version 8 (S4.9): what Home needs to rank and hide.
///
/// - `visits.score` is a visit count that fades with time ([`crate::library::frequency`]), as it
///   stood at `last_visited_ms`. Existing counts start as scores.
/// - `media.hidden_from_home_ms` is when "Remove from Continue Watching" took a media off Home.
///   It comes back once played after that; its progress is never touched.
const HOME: &str = "
    ALTER TABLE visits ADD COLUMN score REAL NOT NULL DEFAULT 0;
    UPDATE visits SET score = count;
    ALTER TABLE media ADD COLUMN hidden_from_home_ms INTEGER;
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
