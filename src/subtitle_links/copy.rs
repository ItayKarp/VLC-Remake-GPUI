//! "Copy next to video" (S6.4.6): a subtitle file copied beside its video as
//! `<video name>.<language>.<extension>` (`Movie.en.srt`, `Movie.en.forced.srt`), the name other
//! players look for too. A VobSub `.idx` takes its `.sub` along.
//!
//! When that name is taken, the user decides per row: Overwrite it, Keep both (the copy gets a
//! number: `Movie.en.2.srt`) or Skip the copy.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::engine::sidecar;

/// The most numbered names tried for Keep both.
const MAX_NUMBER: u32 = 99;
/// The word that marks a forced subtitle in a name.
const FORCED: &str = "forced";

/// What to do when the copy's name is taken.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnConflict {
    /// Replace the file there.
    Overwrite,
    /// Copy under a numbered name.
    #[default]
    KeepBoth,
    /// Don't copy: link the subtitle where it is.
    Skip,
}

/// What copying one subtitle did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Copied {
    /// It was copied here.
    To(PathBuf),
    /// It is already beside the video under the wanted name: nothing to copy.
    AlreadyThere,
    /// Its name was taken and the user chose Skip.
    Skipped,
}

/// Where the subtitle at `subtitle` is copied for the video at `video`.
pub fn destination(video: &Path, subtitle: &Path) -> PathBuf {
    numbered(video, subtitle, None)
}

/// Whether copying `subtitle` for `video` would meet a file of that name that isn't the
/// subtitle itself. Reads the disk.
pub fn conflicts(video: &Path, subtitle: &Path) -> bool {
    let target = destination(video, subtitle);
    exists(&target) && !same_file(&target, subtitle)
}

/// Copies `subtitle` beside `video`, deciding a taken name by `on_conflict`. Blocks on the disk.
pub fn copy(video: &Path, subtitle: &Path, on_conflict: OnConflict) -> io::Result<Copied> {
    let first = destination(video, subtitle);
    if same_file(&first, subtitle) {
        return Ok(Copied::AlreadyThere);
    }
    let target = match (exists(&first), on_conflict) {
        (false, _) | (true, OnConflict::Overwrite) => first,
        (true, OnConflict::Skip) => return Ok(Copied::Skipped),
        (true, OnConflict::KeepBoth) => free_number(video, subtitle)?,
    };
    fs::copy(subtitle, &target)?;
    if let Some(pictures) = vobsub_pictures(subtitle) {
        fs::copy(pictures, target.with_extension("sub"))?;
    }
    Ok(Copied::To(target))
}

/// The copy's name with `number` before the extension, when there is one.
fn numbered(video: &Path, subtitle: &Path, number: Option<u32>) -> PathBuf {
    let folder = video.parent().unwrap_or_else(|| Path::new(""));
    let mut name = video.file_stem().map_or_default(|stem| stem.to_string_lossy().into_owned());
    let described = sidecar::describe(subtitle, video);
    if let Some(language) = described.language {
        name = format!("{name}.{}", language.code());
    }
    if described.forced {
        name = format!("{name}.{FORCED}");
    }
    if let Some(number) = number {
        name = format!("{name}.{number}");
    }
    let extension = subtitle.extension().map_or_default(|e| e.to_string_lossy().to_lowercase());
    folder.join(format!("{name}.{extension}"))
}

/// The first numbered name that is free, from 2 on.
fn free_number(video: &Path, subtitle: &Path) -> io::Result<PathBuf> {
    (2..=MAX_NUMBER)
        .map(|number| numbered(video, subtitle, Some(number)))
        .find(|candidate| !exists(candidate))
        .ok_or_else(|| io::Error::new(io::ErrorKind::AlreadyExists, "every name is taken"))
}

/// The `.sub` beside a VobSub `.idx`.
fn vobsub_pictures(subtitle: &Path) -> Option<PathBuf> {
    let is_idx = subtitle.extension().is_some_and(|e| e.eq_ignore_ascii_case("idx"));
    let pictures = subtitle.with_extension("sub");
    (is_idx && pictures.is_file()).then_some(pictures)
}

fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Whether `a` and `b` are the same file, comparing real paths where they can be read.
fn same_file(a: &Path, b: &Path) -> bool {
    let real = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    real(a) == real(b)
}

/// Why a copy failed, in words a row shows.
pub fn failure_message(error: &io::Error) -> String {
    match error.kind() {
        io::ErrorKind::PermissionDenied | io::ErrorKind::ReadOnlyFilesystem => {
            tr!("Can't write next to this video.").to_owned()
        }
        io::ErrorKind::StorageFull => tr!("The disk is full.").to_owned(),
        io::ErrorKind::NotFound => tr!("The file is gone.").to_owned(),
        _ => tr!("Couldn't copy it: {error}.", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";

    #[test]
    fn the_copy_is_named_after_the_video_and_language() {
        let video = Path::new("/v/The_Wand_S01E01.mp4");
        let name = |subtitle: &str| destination(video, Path::new(subtitle));
        assert_eq!(name("/s/The_Wand_S01E01_English.srt"), Path::new("/v/The_Wand_S01E01.en.srt"));
        assert_eq!(name("/s/Other.forced.ASS"), Path::new("/v/The_Wand_S01E01.forced.ass"));
        assert_eq!(name("/s/x.srt"), Path::new("/v/The_Wand_S01E01.srt"));
    }

    #[test]
    fn a_taken_name_is_overwritten_kept_or_skipped() {
        let (videos, subtitles) = (scratch_dir(), scratch_dir());
        let video = videos.join("Movie.mkv");
        let subtitle = subtitles.join("Movie.English.srt");
        fs::write(&subtitle, SRT).unwrap();
        fs::write(videos.join("Movie.en.srt"), "old").unwrap();
        assert!(conflicts(&video, &subtitle));

        assert_eq!(copy(&video, &subtitle, OnConflict::Skip).unwrap(), Copied::Skipped);
        let kept = copy(&video, &subtitle, OnConflict::KeepBoth).unwrap();
        assert_eq!(kept, Copied::To(videos.join("Movie.en.2.srt")));
        copy(&video, &subtitle, OnConflict::Overwrite).unwrap();
        assert_eq!(fs::read_to_string(videos.join("Movie.en.srt")).unwrap(), SRT);
    }

    #[test]
    fn a_subtitle_already_in_place_isnt_copied() {
        let folder = scratch_dir();
        let subtitle = folder.join("Movie.en.srt");
        fs::write(&subtitle, SRT).unwrap();
        let copied = copy(&folder.join("Movie.mkv"), &subtitle, OnConflict::KeepBoth);
        assert_eq!(copied.unwrap(), Copied::AlreadyThere);
        assert!(!conflicts(&folder.join("Movie.mkv"), &subtitle));
    }
}
