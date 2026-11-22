//! Library preferences (Stage 4, Settings → Library, S5.9): how files and folders open, where
//! titles and artwork come from, and whether the media library is used at all.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Library preferences (Stage 4, S5.9).
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Library"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LibrarySettings {
    /// Lets every launch open its own window instead of handing its files to the
    /// running one (S0.6.1). The opposite of "Only allow one VLC window" (S5.9.1).
    #[serde(deserialize_with = "lenient")]
    pub allow_multiple_instances: bool,
    /// "Apply to files opened from the file manager" (S5.9.1): whether a launch the file manager
    /// started (`--started-from-file`) also hands its files to the running window. Off, those
    /// launches open a window of their own. Counts only while one window is allowed.
    #[serde(deserialize_with = "lenient")]
    pub one_window_from_file_manager: bool,
    /// "When opening a folder: Include subfolders" (S5.9): whether opening a folder, or a
    /// file's folder (S3.1.3), takes the media in its subfolders too.
    #[serde(deserialize_with = "lenient")]
    pub include_subfolders: bool,
    /// "Show hidden files" (S5.9.1): whether the library indexes dot files and files the OS
    /// marks hidden or system (S4.2.2).
    #[serde(deserialize_with = "lenient")]
    pub show_hidden_files: bool,
    /// "Add to the playlist instead of replacing it" (S4.10.6, S5.9.1): whether files handed to
    /// the running window, and library items opened while something plays, go to the end of
    /// the queue rather than replacing it.
    #[serde(deserialize_with = "lenient")]
    pub add_instead_of_replacing: bool,
    /// "Fetch titles and artwork" (S5.9.2): where the library looks for titles and covers.
    /// The internet is used only while Privacy allows it (S5.5.2).
    #[serde(deserialize_with = "lenient")]
    pub artwork_source: ArtworkSource,
    /// "Use the media library" (S5.9.2, S5.9.4): off, the library's views are hidden and its
    /// folders aren't indexed; what was indexed is kept unless the user deletes it.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
}

impl Default for LibrarySettings {
    fn default() -> Self {
        Self {
            allow_multiple_instances: false,
            one_window_from_file_manager: true,
            include_subfolders: true,
            show_hidden_files: false,
            add_instead_of_replacing: false,
            artwork_source: ArtworkSource::default(),
            enabled: true,
        }
    }
}

/// Where the library looks for titles and artwork.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkSource {
    /// Only the tags and pictures inside the file.
    File,
    /// The file, then a cover image in its folder (`cover.jpg`, `folder.png`, …).
    #[default]
    Folder,
    /// The file, its folder, then the internet, while Privacy allows it.
    Internet,
}
