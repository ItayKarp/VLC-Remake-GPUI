//! Passwords out of the database (S9.7.3). Stream links are stored without their user name and
//! password ([`crate::app::media::Media::to_stored`]); this migration takes them out of what
//! builds before that wrote: the history, saved playlists, subtitle and audio sync, bookmarks
//! and the network stream lists. SQLite can't take a URL apart, so the migration calls
//! [`without_user_info`] through an SQL function every connection registers.

use rusqlite::Connection;
use rusqlite::functions::FunctionFlags;

use crate::network::links::without_user_info;

/// The SQL function's name.
const FUNCTION: &str = "without_user_info";

/// Version 15: every stored link without its user name and password. `OR REPLACE` lets a row
/// that now matches one stored without a password take its place. Identities are `url:` and the
/// link.
pub(super) const MIGRATION: &str = "
    UPDATE OR REPLACE media SET
        location = without_user_info(location),
        identity = 'url:' || without_user_info(substr(identity, 5))
        WHERE identity LIKE 'url:%@%';
    UPDATE OR REPLACE playlist_items SET location = without_user_info(location)
        WHERE location LIKE '%://%@%';
    UPDATE OR REPLACE subtitle_sync SET media = without_user_info(media)
        WHERE media LIKE '%://%@%';
    UPDATE OR REPLACE subtitle_links SET media = without_user_info(media)
        WHERE media LIKE '%://%@%';
    UPDATE OR REPLACE audio_delay SET media = without_user_info(media)
        WHERE media LIKE '%://%@%';
    UPDATE OR REPLACE bookmarks SET identity = 'url:' || without_user_info(substr(identity, 5))
        WHERE identity LIKE 'url:%@%';
    DELETE FROM network_streams WHERE url != without_user_info(url);
";

/// Makes [`FUNCTION`] callable on `connection`.
pub(super) fn register(connection: &Connection) -> rusqlite::Result<()> {
    let flags = FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC;
    connection.create_scalar_function(FUNCTION, 1, flags, |context| {
        let text: String = context.get(0)?;
        Ok(without_user_info(&text))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::MIGRATIONS;

    #[test]
    fn stored_passwords_are_taken_out() {
        let connection = Connection::open_in_memory().expect("a database");
        register(&connection).expect("the function");
        for sql in MIGRATIONS.iter().take_while(|&&sql| sql != MIGRATION) {
            connection.execute_batch(sql).expect("an older schema");
        }
        connection
            .execute_batch(
                "INSERT INTO media (identity, location) VALUES
                    ('url:https://me:pw@host/a', 'https://me:pw@host/a'),
                    ('url:udp://10.0.0.1@239.0.0.1:1234', 'udp://10.0.0.1@239.0.0.1:1234'),
                    ('abc', '/home/me@work/a.mkv');
                 INSERT INTO playlists (id, title, created_ms, modified_ms) VALUES (1, 'x', 0, 0);
                 INSERT INTO playlist_items VALUES (1, 0, 'rtsp://u:p@cam/1');",
            )
            .expect("older rows");
        connection.execute_batch(MIGRATION).expect("the scrub");

        let rows: Vec<(String, String)> = connection
            .prepare("SELECT identity, location FROM media ORDER BY id")
            .and_then(|mut query| {
                query.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?.collect()
            })
            .expect("the rows");
        let expected = [
            ("url:https://host/a", "https://host/a"),
            ("url:udp://10.0.0.1@239.0.0.1:1234", "udp://10.0.0.1@239.0.0.1:1234"),
            ("abc", "/home/me@work/a.mkv"),
        ];
        let expected: Vec<(String, String)> =
            expected.iter().map(|(a, b)| ((*a).to_owned(), (*b).to_owned())).collect();
        assert_eq!(rows, expected);
        let item: String = connection
            .query_row("SELECT location FROM playlist_items", [], |row| row.get(0))
            .expect("the item");
        assert_eq!(item, "rtsp://cam/1");
    }
}
