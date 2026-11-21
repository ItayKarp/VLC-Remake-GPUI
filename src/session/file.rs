//! `session.toml`: a [`Snapshot`] as TOML. Media is kept in its stored form (a URL as written, a
//! path losslessly, as Open Recent keeps it), and the position in milliseconds.
//!
//! An item that can't be read back is dropped; when it was the current one, nothing is current.
//! A file that doesn't parse is ignored with a warning: the session is a convenience, and the
//! next save replaces it.

use std::io;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::Snapshot;
use crate::app::media::Media;
use crate::queue::Repeat;

/// The file's layout.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct SessionFile {
    repeat: Repeat,
    shuffle: bool,
    playing: bool,
    current: Option<usize>,
    position_ms: u64,
    items: Vec<String>,
}

/// Reads the session at `path`; `None` when there is none or it can't be read.
pub(super) fn read(path: &Path) -> Option<Snapshot> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!("session: couldn't read the last session: {error}");
            return None;
        }
    };
    parse(&text)
}

/// The snapshot in `text`; `None` when it doesn't parse.
fn parse(text: &str) -> Option<Snapshot> {
    match toml::from_str::<SessionFile>(text) {
        Ok(file) => Some(file.into_snapshot()),
        Err(error) => {
            tracing::warn!("session: the last session couldn't be read, so it's ignored: {error}");
            None
        }
    }
}

/// `snapshot` as the file's text.
pub(super) fn to_text(snapshot: &Snapshot) -> Result<String, toml::ser::Error> {
    let millis = u64::try_from(snapshot.position.as_millis()).unwrap_or(u64::MAX);
    let file = SessionFile {
        repeat: snapshot.repeat,
        shuffle: snapshot.shuffle,
        playing: snapshot.playing,
        current: snapshot.current,
        position_ms: millis,
        items: snapshot.media.iter().map(Media::to_stored).collect(),
    };
    toml::to_string(&file)
}

impl SessionFile {
    fn into_snapshot(self) -> Snapshot {
        let mut current = None;
        let mut media = Vec::with_capacity(self.items.len());
        for (index, item) in self.items.iter().enumerate() {
            let Some(item) = Media::from_stored(item) else { continue };
            if self.current == Some(index) {
                current = Some(media.len());
            }
            media.push(item);
        }
        Snapshot {
            media,
            current,
            position: Duration::from_millis(self.position_ms),
            repeat: self.repeat,
            shuffle: self.shuffle,
            playing: self.playing,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn snapshot() -> Snapshot {
        Snapshot {
            media: vec![
                Media::File(std::env::temp_dir().join("café #1.mkv")),
                Media::Url("https://example.com/live.m3u8".into()),
            ],
            current: Some(1),
            position: Duration::from_millis(83_250),
            repeat: Repeat::All,
            shuffle: true,
            playing: true,
        }
    }

    #[test]
    fn round_trips() {
        let text = to_text(&snapshot()).expect("serializes");
        assert_eq!(parse(&text), Some(snapshot()));
    }

    #[test]
    fn an_unreadable_item_is_dropped_and_the_current_one_follows() {
        let text = "current = 2\nitems = [\"/a.mkv\", \"os-path:nonsense\", \"/c.mkv\"]\n";
        let restored = parse(text).expect("parses");
        let expected: Vec<Media> =
            ["/a.mkv", "/c.mkv"].iter().map(|path| Media::File(PathBuf::from(path))).collect();
        assert_eq!(restored.media, expected);
        assert_eq!(restored.current, Some(1));
    }

    #[test]
    fn missing_keys_take_defaults_and_garbage_is_ignored() {
        assert_eq!(parse(""), Some(Snapshot::default()));
        assert_eq!(parse("repeat = \"sideways\""), None);
    }
}
