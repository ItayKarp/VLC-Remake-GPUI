//! What Properties & Metadata shows (S4.3.5): a media file's tags, technical details and cover
//! source, read from the library, as label and value rows. Read-only: tag editing is a later
//! story.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension as _};

use super::display;
use crate::db::artwork::{self, ArtworkSource};
use crate::db::media::{self, MediaId};
use crate::db::metadata::{self, MediaKind, Metadata};
use crate::history::policy::clock;
use crate::settings::{display_path, stored_path};

/// Bytes in a kilobyte, as file managers count them.
const KILO: f64 = 1000.;
/// Size units after bytes.
const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];

/// What the library knows about one file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Properties {
    /// The file.
    pub path: PathBuf,
    /// Its size in bytes, if it could be read.
    pub size: Option<u64>,
    /// Its tags and technical details; empty when the library hasn't read it.
    pub metadata: Metadata,
    /// Its length.
    pub duration: Option<Duration>,
    /// Where its cover came from, if it has one.
    pub artwork: Option<ArtworkSource>,
    /// When the library first found it, in milliseconds since the Unix epoch, if known.
    pub added_ms: Option<i64>,
}

/// Reads what the library knows about the file at `path`. Blocks on the disk.
pub fn read(connection: &Connection, path: &Path) -> rusqlite::Result<Properties> {
    let size = fs::metadata(path).ok().map(|metadata| metadata.len());
    let mut properties = Properties { path: path.to_path_buf(), size, ..Properties::default() };
    let Some(located) = media::find_at(connection, &stored_path::encode(path))? else {
        return Ok(properties);
    };
    properties.metadata = metadata::get(connection, located.id)?;
    properties.duration = media::saved(connection, located.id)?.duration;
    properties.artwork = artwork::of_media(connection, located.id)?.map(|art| art.source);
    properties.added_ms = added_ms(connection, located.id)?;
    Ok(properties)
}

/// When row `id` was added to the library; rows from before that was recorded hold 0.
fn added_ms(connection: &Connection, id: MediaId) -> rusqlite::Result<Option<i64>> {
    let added: Option<Option<i64>> = connection
        .query_row("SELECT added_ms FROM media WHERE id = ?1", [id.0], |row| row.get(0))
        .optional()?;
    Ok(added.flatten().filter(|&millis| millis > 0))
}

/// The rows to show, in order. Rows for what isn't known are left out, except the file's own.
pub fn rows(properties: &Properties) -> Vec<(&'static str, String)> {
    let path = &properties.path;
    let tags = &properties.metadata;
    let folder = path.parent().map(display_path);
    let optional = [
        (tr!("Artist"), tags.artist.clone()),
        (tr!("Album"), tags.album.clone()),
        (tr!("Album artist"), tags.album_artist.clone()),
        (tr!("Track"), tags.track_number.map(|number| number.to_string())),
        (tr!("Disc"), tags.disc_number.map(|number| number.to_string())),
        (tr!("Genre"), tags.genre.clone()),
        (tr!("Year"), tags.year.map(|year| year.to_string())),
        (tr!("Kind"), kind(tags.kind).map(Into::into)),
        (tr!("Length"), properties.duration.map(clock)),
        (tr!("Resolution"), tags.resolution.map(|r| format!("{} × {}", r.width, r.height))),
        (tr!("Codecs"), tags.codecs.clone()),
        (tr!("Cover"), properties.artwork.map(|source| cover(source).into())),
        (tr!("Size"), properties.size.map(size)),
        (tr!("Folder"), folder),
        (tr!("Date added"), properties.added_ms.map(crate::i18n::date)),
    ];
    let mut rows = vec![
        (tr!("Title"), display::title(tags, path)),
        (tr!("File"), path.file_name().unwrap_or_default().to_string_lossy().into_owned()),
    ];
    rows.extend(optional.into_iter().filter_map(|(label, value)| Some((label, value?))));
    rows
}

fn kind(kind: MediaKind) -> Option<&'static str> {
    match kind {
        MediaKind::Unknown => None,
        MediaKind::Video => Some(tr!("Video")),
        MediaKind::Audio => Some(tr!("Audio")),
    }
}

fn cover(source: ArtworkSource) -> &'static str {
    match source {
        ArtworkSource::Embedded => tr!("In the file"),
        ArtworkSource::Folder => tr!("From the folder"),
        ArtworkSource::Online => tr!("From the internet"),
    }
}

/// A file size as file managers show it: `532 bytes`, `1.4 MB`.
pub fn size(bytes: u64) -> String {
    let mut value = bytes as f64;
    if value < KILO {
        return trn!("{count} byte", "{count} bytes", bytes);
    }
    let mut unit = UNITS[0];
    for next in UNITS {
        value /= KILO;
        unit = next;
        if value < KILO {
            break;
        }
    }
    format!("{} {unit}", crate::i18n::decimal(value, 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::metadata::Resolution;

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(size(532), "532 bytes");
        assert_eq!(size(1_400_000), "1.4 MB");
        assert_eq!(size(2_500_000_000), "2.5 GB");
    }

    fn labels(rows: Vec<(&'static str, String)>) -> Vec<&'static str> {
        rows.into_iter().map(|(label, _)| label).collect()
    }

    #[test]
    fn unknown_values_are_left_out_and_known_ones_listed_in_order() {
        let path = PathBuf::from("/m/Show.S01E02.mkv");
        let bare = Properties { path: path.clone(), ..Properties::default() };
        assert_eq!(labels(rows(&bare)), ["Title", "File", "Folder"]);
        assert_eq!(rows(&bare)[0].1, "Show S01E02");
        let metadata = Metadata {
            kind: MediaKind::Video,
            resolution: Some(Resolution { width: 1920, height: 1080 }),
            codecs: Some("H.264 / AAC".into()),
            ..Metadata::default()
        };
        let full = Properties {
            path,
            size: Some(1_400_000),
            metadata,
            duration: Some(Duration::from_mins(25)),
            artwork: Some(ArtworkSource::Embedded),
            added_ms: None,
        };
        let shown = rows(&full);
        assert_eq!(
            labels(shown.clone()),
            ["Title", "File", "Kind", "Length", "Resolution", "Codecs", "Cover", "Size", "Folder"]
        );
        assert_eq!(shown[4].1, "1920 × 1080");
        assert_eq!(shown[3].1, "25:00");
    }

    #[test]
    fn the_date_added_comes_last_once_known() {
        let path = PathBuf::from("/m/Song.mp3");
        let added_ms = 1_764_000_000_000;
        let known = Properties { path, added_ms: Some(added_ms), ..Properties::default() };
        let shown = rows(&known);
        assert_eq!(labels(shown.clone()), ["Title", "File", "Folder", "Date added"]);
        assert_eq!(shown[3].1, crate::i18n::date(added_ms));
    }
}
