//! What Properties & Metadata shows (S4.3.5): a media file's tags, technical details and cover
//! source, read from the library, as label and value rows. Read-only: tag editing is a later
//! story.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;

use super::display;
use crate::db::artwork::{self, ArtworkSource};
use crate::db::media;
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
    Ok(properties)
}

/// The rows to show, in order. Rows for what isn't known are left out, except the file's own.
pub fn rows(properties: &Properties) -> Vec<(&'static str, String)> {
    let path = &properties.path;
    let tags = &properties.metadata;
    let folder = path.parent().map(display_path);
    let optional = [
        ("Artist", tags.artist.clone()),
        ("Album", tags.album.clone()),
        ("Album artist", tags.album_artist.clone()),
        ("Track", tags.track_number.map(|number| number.to_string())),
        ("Disc", tags.disc_number.map(|number| number.to_string())),
        ("Genre", tags.genre.clone()),
        ("Year", tags.year.map(|year| year.to_string())),
        ("Kind", kind(tags.kind).map(Into::into)),
        ("Length", properties.duration.map(clock)),
        ("Resolution", tags.resolution.map(|r| format!("{} × {}", r.width, r.height))),
        ("Codecs", tags.codecs.clone()),
        ("Cover", properties.artwork.map(|source| cover(source).into())),
        ("Size", properties.size.map(size)),
        ("Folder", folder),
    ];
    let mut rows = vec![
        ("Title", display::title(tags, path)),
        ("File", path.file_name().unwrap_or_default().to_string_lossy().into_owned()),
    ];
    rows.extend(optional.into_iter().filter_map(|(label, value)| Some((label, value?))));
    rows
}

fn kind(kind: MediaKind) -> Option<&'static str> {
    match kind {
        MediaKind::Unknown => None,
        MediaKind::Video => Some("Video"),
        MediaKind::Audio => Some("Audio"),
    }
}

fn cover(source: ArtworkSource) -> &'static str {
    match source {
        ArtworkSource::Embedded => "In the file",
        ArtworkSource::Folder => "From the folder",
        ArtworkSource::Online => "From the internet",
    }
}

/// A file size as file managers show it: `532 bytes`, `1.4 MB`.
pub fn size(bytes: u64) -> String {
    let mut value = bytes as f64;
    if value < KILO {
        return format!("{bytes} bytes");
    }
    let mut unit = UNITS[0];
    for next in UNITS {
        value /= KILO;
        unit = next;
        if value < KILO {
            break;
        }
    }
    format!("{value:.1} {unit}")
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
        };
        let shown = rows(&full);
        assert_eq!(
            labels(shown.clone()),
            ["Title", "File", "Kind", "Length", "Resolution", "Codecs", "Cover", "Size", "Folder"]
        );
        assert_eq!(shown[4].1, "1920 × 1080");
        assert_eq!(shown[3].1, "25:00");
    }
}
