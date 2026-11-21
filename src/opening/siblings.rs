//! Opening one file with the rest of its folder (S3.1.3): the folder's media is queued in
//! natural order and the opened file starts, so Next plays the episode after it.
//!
//! By default only TV episodes do this ([`QueueSiblings::Episodes`]): a file named like
//! `Show.S01E02.mkv` or `Show 1x02.mkv` brings the folder's other episodes along, and the
//! folder's other files (a sample, extras, an unrelated download) stay out. Settings → Library
//! "Include subfolders" decides whether subfolders count as the folder.

use std::path::{Path, PathBuf};

use super::Cancel;
use super::scan::{self, Subfolders};
use crate::settings::QueueSiblings;

/// The files to queue for opening `file`, and the index of `file` among them. `None` when the
/// policy leaves the file alone or its folder has nothing else to add.
pub fn queue(
    file: &Path,
    policy: QueueSiblings,
    subfolders: Subfolders,
    cancel: &Cancel,
) -> Option<(Vec<PathBuf>, usize)> {
    let name = file.file_name()?.to_string_lossy();
    let only_episodes = match policy {
        QueueSiblings::Never => return None,
        QueueSiblings::Episodes if !is_episode(&name) => return None,
        QueueSiblings::Episodes => true,
        QueueSiblings::Always => false,
    };
    let mut files = scan::scan(file.parent()?, subfolders, cancel).ok()?.files;
    if only_episodes {
        files.retain(|path| path.file_name().is_some_and(|n| is_episode(&n.to_string_lossy())));
    }
    let start = files.iter().position(|path| path == file)?;
    (files.len() > 1).then_some((files, start))
}

/// Whether `name` reads like a TV episode: `S01E02` or `1x02`, at the start of a word, in any
/// case.
pub fn is_episode(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let bytes = name.as_bytes();
    (0..bytes.len()).any(|i| {
        let starts_word = i == 0 || !bytes[i - 1].is_ascii_alphanumeric();
        starts_word && (season_episode(&bytes[i..]) || season_cross_episode(&bytes[i..]))
    })
}

/// `s01e02` (one or two season digits, one to three episode digits).
fn season_episode(text: &[u8]) -> bool {
    let Some(rest) = text.strip_prefix(b"s") else { return false };
    let (season, rest) = digits(rest);
    let Some(rest) = rest.strip_prefix(b"e") else { return false };
    (1..=2).contains(&season) && (1..=3).contains(&digits(rest).0)
}

/// `1x02` as a word of its own, so `1920x1080` isn't one.
fn season_cross_episode(text: &[u8]) -> bool {
    let (season, rest) = digits(text);
    let Some(rest) = rest.strip_prefix(b"x") else { return false };
    let (episode, rest) = digits(rest);
    let ends_word = rest.first().is_none_or(|c| !c.is_ascii_alphanumeric());
    (1..=2).contains(&season) && (2..=3).contains(&episode) && ends_word
}

/// How many ASCII digits `text` starts with, and what follows them.
fn digits(text: &[u8]) -> (usize, &[u8]) {
    let count = text.iter().take_while(|c| c.is_ascii_digit()).count();
    (count, &text[count..])
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::opening::scratch::scratch_dir;

    fn names(files: &[PathBuf]) -> Vec<String> {
        files.iter().map(|f| f.file_name().unwrap().to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn episode_names() {
        for name in ["Show.S01E02.mkv", "show s1e2.mp4", "Show 1x02.avi", "S10E100 Finale.mkv"] {
            assert!(is_episode(name), "{name}");
        }
        for name in ["movie.mkv", "1920x1080.mp4", "bus01e02.mkv", "s123e01.mkv", "Show 1x2.mkv"] {
            assert!(!is_episode(name), "{name}");
        }
    }

    #[test]
    fn episodes_queue_the_other_episodes_in_natural_order() {
        let dir = scratch_dir();
        for name in ["Show S01E10.mkv", "Show S01E02.mkv", "sample.mkv", "Show S01E01.mkv"] {
            fs::write(dir.join(name), b"x").unwrap();
        }
        let file = dir.join("Show S01E02.mkv");
        let cancel = Cancel::default();
        let (files, start) =
            queue(&file, QueueSiblings::Episodes, Subfolders::Include, &cancel).unwrap();
        assert_eq!(names(&files), ["Show S01E01.mkv", "Show S01E02.mkv", "Show S01E10.mkv"]);
        assert_eq!(start, 1);
        let (files, start) =
            queue(&file, QueueSiblings::Always, Subfolders::Include, &cancel).unwrap();
        assert_eq!(files.len(), 4);
        assert_eq!(files[start], file);
        assert!(queue(&file, QueueSiblings::Never, Subfolders::Include, &cancel).is_none());
    }

    #[test]
    fn movies_and_lone_files_stay_alone() {
        let dir = scratch_dir();
        fs::write(dir.join("movie.mkv"), b"x").unwrap();
        fs::write(dir.join("other.mkv"), b"x").unwrap();
        fs::write(dir.join("Show S01E01.mkv"), b"x").unwrap();
        let cancel = Cancel::default();
        let policy = QueueSiblings::Episodes;
        assert!(queue(&dir.join("movie.mkv"), policy, Subfolders::Include, &cancel).is_none());
        let episode = dir.join("Show S01E01.mkv");
        assert!(queue(&episode, policy, Subfolders::Include, &cancel).is_none());
    }

    #[test]
    fn subfolders_follow_the_setting() {
        let dir = scratch_dir();
        fs::create_dir(dir.join("Season 2")).unwrap();
        fs::write(dir.join("Show S01E01.mkv"), b"x").unwrap();
        fs::write(dir.join("Season 2/Show S02E01.mkv"), b"x").unwrap();
        let file = dir.join("Show S01E01.mkv");
        let cancel = Cancel::default();
        let policy = QueueSiblings::Episodes;
        assert_eq!(queue(&file, policy, Subfolders::Include, &cancel).unwrap().0.len(), 2);
        assert!(queue(&file, policy, Subfolders::Skip, &cancel).is_none());
    }
}
