//! What the Add Subtitles dialog holds, apart from how it looks: the targets and origins its
//! dropdowns offer, and each video's linked and chosen subtitles, from which Save works out what
//! to link and unlink.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use gpui::SharedString;

use crate::db::playlists::PlaylistId;
use crate::subtitle_links::copy::OnConflict;
use crate::subtitle_links::{Addition, Change};

/// What the dialog adds subtitles to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::main_window) enum Target {
    /// One video.
    File(PathBuf),
    /// The videos in a folder, not its subfolders.
    Folder(PathBuf),
    /// A saved playlist's local files.
    Playlist { id: PlaylistId, title: SharedString },
}

impl Target {
    /// The Target dropdown's label for it.
    pub(super) fn label(&self) -> &'static str {
        match self {
            Self::File(_) => "To This File",
            Self::Folder(_) => "To This Folder",
            Self::Playlist { .. } => "To This Playlist",
        }
    }

    /// The targets a dialog opened on `self` offers: a file's folder too.
    pub(super) fn offered(self) -> Vec<Self> {
        match &self {
            Self::File(path) => {
                let folder = path.parent().map(|folder| Self::Folder(folder.to_path_buf()));
                std::iter::once(self.clone()).chain(folder).collect()
            }
            _ => vec![self],
        }
    }
}

/// Where the subtitles come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Origin {
    /// Files in a folder on this computer.
    Computer,
    /// A search of OpenSubtitles (S6.5).
    OpenSubtitles,
    /// One file downloaded from a link.
    Link,
}

impl Origin {
    /// The Origin dropdown's label for it.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Computer => "From Computer",
            Self::OpenSubtitles => "From OpenSubtitles",
            Self::Link => "From Link …",
        }
    }

    /// The origins `target` can take: a search or a link finds files for one video only.
    pub(super) fn offered(target: &Target) -> Vec<Self> {
        match target {
            Target::File(_) => vec![Self::Computer, Self::OpenSubtitles, Self::Link],
            _ => vec![Self::Computer],
        }
    }
}

/// A video and a subtitle file: what a conflict or an error belongs to.
pub(super) type Pair = (PathBuf, PathBuf);

/// One video's subtitles: the ones linked to it, and the ones the dialog will leave linked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Choice {
    pub(super) video: PathBuf,
    /// Linked to it in the database when the dialog opened (or saved since).
    pub(super) linked: Vec<PathBuf>,
    /// Linked to it once Save is pressed, in order.
    pub(super) chosen: Vec<PathBuf>,
}

impl Choice {
    /// `video` with its `linked` subtitles, choosing them, or `proposed` when there are none.
    pub(super) fn new(video: PathBuf, linked: Vec<PathBuf>, proposed: Vec<PathBuf>) -> Self {
        let chosen = if linked.is_empty() { proposed } else { linked.clone() };
        Self { video, linked, chosen }
    }

    /// Whether `subtitle` will be linked.
    pub(super) fn is_chosen(&self, subtitle: &Path) -> bool {
        self.chosen.iter().any(|chosen| chosen == subtitle)
    }

    /// Chooses `subtitle`, or leaves it out when it was chosen.
    pub(super) fn toggle(&mut self, subtitle: &Path) {
        if self.is_chosen(subtitle) {
            self.chosen.retain(|chosen| chosen != subtitle);
        } else {
            self.chosen.push(subtitle.to_path_buf());
        }
    }

    /// The chosen subtitles that aren't linked yet.
    pub(super) fn additions(&self) -> impl Iterator<Item = &PathBuf> {
        self.chosen.iter().filter(|chosen| !self.linked.contains(chosen))
    }

    /// The linked subtitles no longer chosen.
    fn removals(&self) -> impl Iterator<Item = &PathBuf> {
        self.linked.iter().filter(|linked| !self.chosen.contains(linked))
    }

    /// What Save changes for this video; `None` when nothing. `copy` decides how each addition
    /// is copied next to the video (`None` links it where it is).
    pub(super) fn change(&self, copy: impl Fn(&Pair) -> Option<OnConflict>) -> Option<Change> {
        let add: Vec<Addition> = self
            .additions()
            .map(|subtitle| Addition {
                subtitle: subtitle.clone(),
                copy: copy(&(self.video.clone(), subtitle.clone())),
            })
            .collect();
        let remove: Vec<PathBuf> = self.removals().cloned().collect();
        (!add.is_empty() || !remove.is_empty()).then(|| Change {
            video: self.video.clone(),
            add,
            remove,
        })
    }

    /// Records that `subtitle` (chosen as `chosen`) is now linked as `linked`, a copy maybe, so
    /// saving again doesn't link it twice.
    pub(super) fn saved(&mut self, chosen: &Path, linked: &Path) {
        for slot in self.chosen.iter_mut().filter(|slot| *slot == chosen) {
            *slot = linked.to_path_buf();
        }
        if !self.linked.iter().any(|known| known == linked) {
            self.linked.push(linked.to_path_buf());
        }
    }

    /// Records that the removals were saved.
    pub(super) fn removals_saved(&mut self) {
        let chosen = &self.chosen;
        self.linked.retain(|linked| chosen.contains(linked));
    }
}

/// "Copy next to video" and how each taken name is settled.
#[derive(Clone, Debug, Default)]
pub(super) struct Copying {
    /// The checkbox.
    pub(super) on: bool,
    /// The additions whose copy's name is taken, and what the user chose for each.
    pub(super) conflicts: HashMap<Pair, OnConflict>,
    /// The additions checked for taken names; the rest are being checked.
    pub(super) checked: HashSet<Pair>,
}

impl Copying {
    /// How `pair` is copied on Save; `None` while the checkbox is off.
    pub(super) fn copy_of(&self, pair: &Pair) -> Option<OnConflict> {
        self.on.then(|| self.conflicts.get(pair).copied().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> PathBuf {
        PathBuf::from(format!("/s/{name}"))
    }

    #[test]
    fn a_video_with_links_starts_with_them_chosen() {
        let linked = Choice::new(path("v.mkv"), vec![path("a.srt")], vec![path("b.srt")]);
        assert_eq!(linked.chosen, [path("a.srt")]);
        let fresh = Choice::new(path("v.mkv"), Vec::new(), vec![path("b.srt")]);
        assert_eq!(fresh.chosen, [path("b.srt")]);
    }

    #[test]
    fn save_links_the_new_and_unlinks_the_dropped() {
        let mut choice = Choice::new(path("v.mkv"), vec![path("a.srt")], Vec::new());
        assert_eq!(choice.change(|_| None), None);
        choice.toggle(&path("a.srt"));
        choice.toggle(&path("b.srt"));
        let change = choice.change(|_| Some(OnConflict::Skip)).expect("something changed");
        assert_eq!(change.remove, [path("a.srt")]);
        assert_eq!(change.add[0].subtitle, path("b.srt"));
        assert_eq!(change.add[0].copy, Some(OnConflict::Skip));

        choice.saved(&path("b.srt"), &path("v.en.srt"));
        choice.removals_saved();
        assert_eq!(choice.change(|_| None), None);
        assert_eq!(choice.linked, [path("v.en.srt")]);
    }

    #[test]
    fn a_file_also_offers_its_folder_and_only_files_search() {
        let offered = Target::File(path("v.mkv")).offered();
        assert_eq!(offered[1], Target::Folder(PathBuf::from("/s")));
        assert_eq!(Origin::offered(&offered[1]), [Origin::Computer]);
        assert_eq!(Origin::offered(&offered[0]).len(), 3);
    }
}
