//! Opening media (S1.3): what the Open dialogs, drag and drop, paste and Open Recent hand
//! over, turned into items to play.
//!
//! - [`kind`] and [`sniff`]: what a file is, by extension or by content (S1.3.6).
//! - [`scan`]: the media in a folder, asynchronously and cancellably.
//! - [`playlist`]: playlist files expand into their entries (S1.3.4).
//! - [`siblings`]: one file opened with the rest of its folder (S3.1.3).
//! - [`paste`]: paths and URLs from clipboard text (S1.3.3).
//! - [`recent`]: the Open Recent list (S1.3.5).
//! - [`resolve`](resolve::resolve): all of the above for one batch of picked items.
//!
//! Nothing here touches GPUI except [`recent`]; the window runs [`resolve::resolve`] on a
//! background thread and shows the [`Resolved`] outcome.

pub mod kind;
pub mod paste;
pub mod playlist;
pub mod recent;
pub mod resolve;
pub mod scan;
#[cfg(test)]
pub(crate) mod scratch;
pub mod siblings;
mod sniff;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::app::media::Media;

/// Stops a scan or expansion that is no longer wanted (the user pressed Cancel or opened
/// something else). Clones share the flag.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// Asks the work to stop at its next check.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether [`Self::cancel`] was called.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// What one batch of picked items resolved to.
#[derive(Debug, Default)]
pub struct Resolved {
    /// The items to play, in order: folders scanned and playlists expanded.
    pub media: Vec<Media>,
    /// The index in [`Self::media`] to start playing at: the opened file, when its folder was
    /// queued with it (S3.1.3), else 0.
    pub start: usize,
    /// Subtitle files picked along with them, to attach to the video.
    pub subtitles: Vec<PathBuf>,
    /// How many picked files were skipped as not media.
    pub skipped: usize,
    /// The picked items that gave something to play, for Open Recent.
    pub opened: Vec<Media>,
    /// Picked folders and playlists that gave nothing, and why.
    pub problems: Vec<(Media, Problem)>,
}

/// Why a picked folder or playlist gave nothing (or less than everything) to play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// It couldn't be read: missing, no permission (on macOS, the user may have refused access
    /// to Desktop, Documents or Downloads), or the drive went away.
    Unreadable(io::ErrorKind),
    /// The folder has no media files.
    NoMediaInFolder,
    /// The folder has more media files than one scan takes; only the first ones are opened.
    FolderTruncated,
    /// The playlist lists nothing.
    EmptyPlaylist,
    /// The playlist file is too large to be one.
    PlaylistTooLarge,
}

impl Problem {
    /// The message for `item` having this problem.
    pub fn message(self, item: &Media) -> String {
        let name = item.display_name();
        match self {
            Problem::Unreadable(io::ErrorKind::PermissionDenied) => {
                format!("VLC isn't allowed to read “{name}”. Check its access in system settings.")
            }
            Problem::Unreadable(io::ErrorKind::NotFound) => format!("“{name}” no longer exists."),
            Problem::Unreadable(_) => format!("Couldn't read “{name}”."),
            Problem::NoMediaInFolder => format!("No media files in “{name}”."),
            Problem::FolderTruncated => format!(
                "“{name}” has more than {} media files; only the first ones were opened.",
                scan::MAX_FILES
            ),
            Problem::EmptyPlaylist => format!("The playlist “{name}” is empty."),
            Problem::PlaylistTooLarge => format!("“{name}” is too large to be a playlist."),
        }
    }
}
