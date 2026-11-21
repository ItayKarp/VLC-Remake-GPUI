//! Folder artwork (S4.3.2): the cover image an album's folder holds next to its tracks, for
//! tracks without a picture inside them. `cover`, then `folder`, then `front`, then the largest
//! `AlbumArt*` (Windows Media Player writes `AlbumArt_{…}_Large.jpg` and `_Small.jpg`), as JPEG
//! or PNG, in any letter case.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// The names a cover may have, without extension, best first.
const COVER_NAMES: [&str; 3] = ["cover", "folder", "front"];
/// Windows Media Player's cover names start with this.
const ALBUM_ART_PREFIX: &str = "albumart";
/// The extensions a cover may have.
const EXTENSIONS: [&str; 3] = ["jpg", "jpeg", "png"];

/// The cover image in `folder`, if it has one.
pub fn find(folder: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(folder).ok()?;
    let candidates: Vec<(OsString, u64)> = entries
        .flatten()
        .filter_map(|entry| {
            let metadata = entry.metadata().ok().filter(fs::Metadata::is_file)?;
            Some((entry.file_name(), metadata.len()))
        })
        .collect();
    let index = pick(&candidates)?;
    Some(folder.join(&candidates[index].0))
}

/// Which of `files` (name and size) is the folder's cover.
pub fn pick(files: &[(OsString, u64)]) -> Option<usize> {
    let ranked = files.iter().enumerate().filter_map(|(index, (name, size))| {
        let rank = rank(Path::new(name))?;
        Some((rank, std::cmp::Reverse(*size), index))
    });
    ranked.min().map(|(_, _, index)| index)
}

/// How good a cover name is (lower is better), or `None` if it isn't one.
fn rank(name: &Path) -> Option<usize> {
    let extension = name.extension()?.to_str()?.to_lowercase();
    if !EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }
    let stem = name.file_stem()?.to_str()?.to_lowercase();
    if let Some(rank) = COVER_NAMES.iter().position(|cover| *cover == stem) {
        return Some(rank);
    }
    stem.starts_with(ALBUM_ART_PREFIX).then_some(COVER_NAMES.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(names: &[(&str, u64)]) -> Vec<(OsString, u64)> {
        names.iter().map(|(name, size)| (OsString::from(name), *size)).collect()
    }

    #[test]
    fn the_best_named_cover_wins() {
        let found = files(&[("01 Song.mp3", 9), ("Folder.JPG", 5), ("cover.png", 1)]);
        assert_eq!(pick(&found), Some(2));
        assert_eq!(pick(&files(&[("front.jpeg", 1), ("folder.jpg", 1)])), Some(1));
    }

    #[test]
    fn the_largest_album_art_wins() {
        let found = files(&[
            ("AlbumArt_{1}_Small.jpg", 10),
            ("AlbumArt_{1}_Large.jpg", 90),
            ("AlbumArtSmall.jpg", 5),
        ]);
        assert_eq!(pick(&found), Some(1));
    }

    #[test]
    fn other_pictures_and_names_arent_covers() {
        assert_eq!(pick(&files(&[("cover.gif", 1), ("scan.jpg", 1), ("cover", 1)])), None);
        assert_eq!(pick(&[]), None);
    }

    #[test]
    fn finds_the_cover_on_disk() {
        let dir = crate::opening::scratch::scratch_dir();
        fs::write(dir.join("song.mp3"), "x").expect("write");
        assert_eq!(find(&dir), None);
        fs::write(dir.join("Cover.jpg"), "x").expect("write");
        assert_eq!(find(&dir), Some(dir.join("Cover.jpg")));
    }
}
