//! Snapshots (S8.4): the picture on screen saved as an image file.
//!
//! - [`naming`] names the file `<title>_<hh-mm-ss.mmm>.<ext>`, safe on every OS.
//! - [`folder`] says where it goes: Settings → Video's snapshot folder, else `Pictures/VLC`, and
//!   the next place that takes it when that one can't be written.
//! - [`save`] encodes and writes it, off the UI thread, and makes the toast's thumbnail.

pub mod folder;
pub mod naming;
mod save;

use serde::{Deserialize, Serialize};

pub use save::{Request, Saved, save};

/// "Snapshot format": what snapshots are saved as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotFormat {
    /// Lossless.
    #[default]
    Png,
    /// Smaller, lossy.
    Jpeg,
}

impl SnapshotFormat {
    /// The file extension, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
        }
    }

    /// The `image` crate's name for it.
    fn image_format(self) -> image::ImageFormat {
        match self {
            Self::Png => image::ImageFormat::Png,
            Self::Jpeg => image::ImageFormat::Jpeg,
        }
    }
}
