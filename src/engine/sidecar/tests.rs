//! Finding subtitle files in real folders.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use super::*;

const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n";

/// An empty folder of its own for one test.
fn scratch() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rvlc-sidecar-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &Path, name: &str, content: &[u8]) {
    let path = dir.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// The found files' paths below `dir`, with `/` between folders.
fn names(dir: &Path, found: &[Sidecar]) -> Vec<String> {
    found
        .iter()
        .map(|sidecar| {
            let relative = sidecar.path.strip_prefix(dir).unwrap();
            relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect()
}

#[test]
fn files_beside_the_video_and_in_subtitle_folders_load() {
    let dir = scratch();
    write(&dir, "Movie.mkv", b"video");
    for name in
        ["Movie.srt", "Movie.he.srt", "Other.srt", "Subs/Movie.fr.srt", "Subs/Movie/2_English.srt"]
    {
        write(&dir, name, SRT.as_bytes());
    }
    let found = find(&dir.join("Movie.mkv"), FileMatching::TitleWithExtra);
    assert_eq!(
        names(&dir, &found),
        ["Movie.srt", "Movie.he.srt", "Subs/Movie.fr.srt", "Subs/Movie/2_English.srt"]
    );
    let languages: Vec<_> =
        found.iter().map(|sidecar| sidecar.language.as_ref().map(Language::code)).collect();
    assert_eq!(languages, [None, Some("he"), Some("fr"), Some("en")]);
}

#[test]
fn the_matching_level_decides_which_names_load() {
    let dir = scratch();
    write(&dir, "Movie.mkv", b"video");
    for name in ["Movie.srt", "Movie.en.srt", "[Group] Movie.srt", "Other.srt"] {
        write(&dir, name, SRT.as_bytes());
    }
    let count = |level| find(&dir.join("Movie.mkv"), level).len();
    assert_eq!(count(FileMatching::Exact), 1);
    assert_eq!(count(FileMatching::TitleWithExtra), 2);
    assert_eq!(count(FileMatching::ContainingTitle), 3);
    assert_eq!(count(FileMatching::AnyInFolder), 4);
}

#[test]
fn files_that_are_not_subtitles_are_skipped() {
    let dir = scratch();
    write(&dir, "Movie.mkv", b"video");
    write(&dir, "Movie.en.srt", b"\x1A\x45\xDF\xA3\x00\x00binary");
    write(&dir, "Movie.fr.srt", b"");
    write(&dir, "._Movie.srt", SRT.as_bytes());
    write(&dir, "Movie.nfo", SRT.as_bytes());
    assert_eq!(find(&dir.join("Movie.mkv"), FileMatching::AnyInFolder), []);
}

#[test]
fn a_vobsub_pair_loads_once_by_its_index() {
    let dir = scratch();
    write(&dir, "Movie.mkv", b"video");
    write(&dir, "Movie.idx", b"# VobSub index file, v7 (do not modify this line!)\n");
    write(&dir, "Movie.sub", b"\x00\x00\x01\xBA");
    write(&dir, "Lonely.idx", b"# VobSub index file, v7\n");
    let found = find(&dir.join("Movie.mkv"), FileMatching::AnyInFolder);
    assert_eq!(names(&dir, &found), ["Movie.idx"]);
}

#[test]
fn a_missing_folder_finds_nothing() {
    let missing = scratch().join("gone/Movie.mkv");
    assert_eq!(find(&missing, FileMatching::AnyInFolder), []);
}
