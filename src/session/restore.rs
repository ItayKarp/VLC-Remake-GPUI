//! What a launch does with the last session (S3.4.2): the queue comes back as it was, and the
//! current item is loaded paused at its saved position, never played.
//!
//! The current item stays unloaded (current, so Play starts it) when:
//! - it is a URL: a stream or a link that may have expired is only opened when the user plays
//!   it, so its error shows then;
//! - it is a file that isn't there (deleted, or on a drive that's offline): it stays queued,
//!   greyed in the queue panel, rather than greeting the user with an error;
//! - the last run ended while it was playing: it may have crashed the decoder, and loading it
//!   again could crash this launch too, so the user is told instead.

use std::path::Path;
use std::time::Duration;

use super::Snapshot;
use crate::app::media::Media;

/// What happens to the current item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Load {
    /// Nothing is loaded: there's no current item, or it waits for Play.
    Nothing,
    /// The current item opens paused this far in.
    Paused(Duration),
    /// The last run ended while this item played; it waits for Play, and the user is told.
    Crashed(Media),
}

/// The restored session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restore {
    /// The queue, with its current item, Repeat and Shuffle; position and playing are spent.
    pub session: Snapshot,
    /// What happens to the current item.
    pub load: Load,
}

/// Plans restoring `session`; `exists` tells whether a local file is there.
pub fn plan(session: Snapshot, exists: impl Fn(&Path) -> bool) -> Restore {
    let current = session.current.and_then(|index| session.media.get(index));
    let load = match current {
        Some(media) if session.playing => Load::Crashed(media.clone()),
        Some(Media::File(path)) if exists(path) => Load::Paused(session.position),
        _ => Load::Nothing,
    };
    Restore { session, load }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn session(current: Media, playing: bool) -> Snapshot {
        Snapshot {
            media: vec![Media::File(PathBuf::from("/other.mkv")), current],
            current: Some(1),
            position: Duration::from_secs(754),
            playing,
            ..Snapshot::default()
        }
    }

    fn file() -> Media {
        Media::File(PathBuf::from("/movie.mkv"))
    }

    #[test]
    fn a_file_that_is_there_opens_paused_where_it_was() {
        assert_eq!(
            plan(session(file(), false), |_| true).load,
            Load::Paused(Duration::from_secs(754))
        );
    }

    #[test]
    fn missing_files_and_urls_wait_for_play() {
        assert_eq!(plan(session(file(), false), |_| false).load, Load::Nothing);
        let url = Media::Url("https://example.com/expired".into());
        assert_eq!(plan(session(url, false), |_| true).load, Load::Nothing);
    }

    #[test]
    fn the_item_that_was_playing_when_the_app_died_is_not_loaded() {
        assert_eq!(plan(session(file(), true), |_| true).load, Load::Crashed(file()));
    }

    #[test]
    fn nothing_current_loads_nothing() {
        let restored = plan(Snapshot { current: None, ..session(file(), true) }, |_| true);
        assert_eq!(restored.load, Load::Nothing);
        assert_eq!(restored.session.media.len(), 2);
    }
}
