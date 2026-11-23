//! Video's Snapshots (S8.4): where snapshots go and what they are saved as. Nothing is
//! designed for them; they sit with the other video rows as VLC's own preferences have them.
//! A snapshot reads them when it is taken, so changes count from the next one.

use super::super::def::{Control, PathPick, SettingDef};
use super::super::value::{Choice, SettingChoice, choice, text};
use crate::settings::registry::Category;
use crate::snapshot::SnapshotFormat;
use crate::snapshot::folder::DEFAULT_LABEL;

const SNAPSHOTS: (Category, &str) = (Category::Video, "Snapshots");

impl SettingChoice for SnapshotFormat {
    const VALUES: &'static [Self] = &[Self::Png, Self::Jpeg];
    const CHOICES: &'static [Choice] = &[Choice::new("png", "PNG"), Choice::new("jpeg", "JPEG")];
}

/// The Snapshots rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "snapshots_folder",
        SNAPSHOTS,
        "Save snapshots to",
        Control::Path(PathPick::Folder),
        text!(local.snapshots_folder),
    )
    .info(
        "The folder snapshots are saved in, made if it's missing. Leave it empty for a VLC \
         folder in your Pictures folder. When it can't be written, the snapshot goes to the \
         next place that can take it, and VLC says where.",
    )
    .placeholder(DEFAULT_LABEL)
    .keywords(&["snapshot", "screenshot", "folder", "location", "path", "picture"]),
    SettingDef::new(
        "video.snapshot_format",
        SNAPSHOTS,
        "Snapshot format",
        Control::Dropdown(SnapshotFormat::CHOICES),
        choice!(prefs.video.snapshot_format),
    )
    .info("PNG keeps every pixel as it was; JPEG makes much smaller files that look the same.")
    .keywords(&["snapshot", "screenshot", "png", "jpg", "jpeg", "image"]),
];
