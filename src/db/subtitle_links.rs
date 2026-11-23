//! The `subtitle_links` table (S6.4.6, D16): subtitle files the Add Subtitles dialog linked to a
//! media, loaded with it whenever it opens. A link leaves both files where they are; removing it
//! deletes the row, never the file.
//!
//! Rows are keyed by the media's and the subtitle's stored locations
//! ([`crate::settings::stored_path`]). Links aren't history: Clear History keeps them.

use rusqlite::{Connection, params};

use super::now_ms;

/// Version 10 (S6.4.6): the subtitle files linked to each media.
pub(super) const MIGRATION: &str = "
    CREATE TABLE subtitle_links (
        media TEXT NOT NULL,
        subtitle TEXT NOT NULL,
        added_ms INTEGER NOT NULL,
        PRIMARY KEY (media, subtitle)
    );
";

/// The subtitles linked to `media`, oldest link first.
pub fn load(connection: &Connection, media: &str) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare_cached(
        "SELECT subtitle FROM subtitle_links WHERE media = ?1 ORDER BY added_ms, subtitle",
    )?;
    let rows = statement.query_map([media], |row| row.get(0))?;
    rows.collect()
}

/// Links `subtitle` to `media`; linking it again keeps the first link.
pub fn link(connection: &Connection, media: &str, subtitle: &str) -> rusqlite::Result<()> {
    connection.execute(
        "INSERT INTO subtitle_links (media, subtitle, added_ms) VALUES (?1, ?2, ?3)
         ON CONFLICT (media, subtitle) DO NOTHING",
        params![media, subtitle, now_ms()],
    )?;
    Ok(())
}

/// Removes the link of `subtitle` to `media`, if there is one.
pub fn unlink(connection: &Connection, media: &str, subtitle: &str) -> rusqlite::Result<()> {
    connection.execute(
        "DELETE FROM subtitle_links WHERE media = ?1 AND subtitle = ?2",
        params![media, subtitle],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[test]
    fn links_are_added_once_and_removed() {
        let mut database = Database::in_memory();
        let connection = database.connection();
        link(connection, "/m.mkv", "/s/m.en.srt").unwrap();
        link(connection, "/m.mkv", "/s/m.he.srt").unwrap();
        link(connection, "/m.mkv", "/s/m.en.srt").unwrap();
        link(connection, "/other.mkv", "/s/m.en.srt").unwrap();
        assert_eq!(load(connection, "/m.mkv").unwrap().len(), 2);

        unlink(connection, "/m.mkv", "/s/m.en.srt").unwrap();
        assert_eq!(load(connection, "/m.mkv").unwrap(), ["/s/m.he.srt"]);
        assert_eq!(load(connection, "/other.mkv").unwrap(), ["/s/m.en.srt"]);
    }
}
