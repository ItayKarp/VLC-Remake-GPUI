//! Turns what the user picked, dropped or pasted into what to play: folders are scanned,
//! playlists expanded, subtitle files set aside to attach, and other files skipped.
//!
//! Non-media files mixed with media are skipped and counted ("3 files skipped"). When nothing
//! among the picked files is recognizable as media, they are handed to the engine anyway:
//! libVLC probes far more formats than [`super::sniff`] knows, and it gives a clear error for
//! the rest.

use std::fs;
use std::path::{Path, PathBuf};

use super::kind::{self, Kind};
use super::playlist::{self, PlaylistError};
use super::scan;
use super::{Cancel, Problem, Resolved};
use crate::app::media::Media;

/// Resolves `items` in order. Returns `None` when cancelled. Reads the file system, so run it
/// off the UI thread.
pub fn resolve(items: Vec<Media>, cancel: &Cancel) -> Option<Resolved> {
    let mut resolver = Resolver { cancel, resolved: Resolved::default(), others: Vec::new() };
    for item in items {
        if cancel.is_cancelled() {
            return None;
        }
        resolver.add(item);
    }
    (!cancel.is_cancelled()).then(|| resolver.finish())
}

struct Resolver<'a> {
    cancel: &'a Cancel,
    resolved: Resolved,
    /// Picked files that aren't recognizably media.
    others: Vec<PathBuf>,
}

impl Resolver<'_> {
    fn add(&mut self, item: Media) {
        let Media::File(path) = &item else {
            self.play(item.clone(), vec![item]);
            return;
        };
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => self.add_folder(item),
            Ok(_) => self.add_file(item),
            // Missing or unreadable: the engine reports it clearly when it tries.
            Err(_) => self.play(item.clone(), vec![item]),
        }
    }

    fn add_folder(&mut self, item: Media) {
        let path = item.as_path().expect("a folder is a path");
        match scan::scan(path, self.cancel) {
            Ok(scan) if scan.files.is_empty() => self.problem(path, Problem::NoMediaInFolder),
            Ok(scan) => {
                if scan.truncated {
                    self.problem(path, Problem::FolderTruncated);
                }
                self.play(item, scan.files.into_iter().map(Media::File).collect());
            }
            Err(error) => self.problem(path, Problem::Unreadable(error.kind())),
        }
    }

    fn add_file(&mut self, item: Media) {
        let path = item.as_path().expect("a file is a path").to_path_buf();
        match kind::classify(&path) {
            Ok(Kind::Media) => self.play(item.clone(), vec![item]),
            Ok(Kind::Playlist(format)) => match playlist::expand(&path, format, self.cancel) {
                Ok(items) => self.play(item, items),
                Err(PlaylistError::Empty) => self.problem(&path, Problem::EmptyPlaylist),
                Err(PlaylistError::TooLarge) => self.problem(&path, Problem::PlaylistTooLarge),
                Err(PlaylistError::Unreadable(kind)) => {
                    self.problem(&path, Problem::Unreadable(kind));
                }
            },
            Ok(Kind::Subtitle) => self.resolved.subtitles.push(path),
            Ok(Kind::Other) => self.others.push(path),
            Err(error) => self.problem(&path, Problem::Unreadable(error.kind())),
        }
    }

    /// Plays `media`, which came from `opened` (remembered in Open Recent).
    fn play(&mut self, opened: Media, media: Vec<Media>) {
        self.resolved.media.extend(media);
        self.resolved.opened.push(opened);
    }

    fn problem(&mut self, path: &Path, problem: Problem) {
        self.resolved.problems.push((Media::File(path.to_path_buf()), problem));
    }

    fn finish(mut self) -> Resolved {
        if self.resolved.media.is_empty() {
            for path in self.others {
                self.resolved.media.push(Media::File(path.clone()));
                self.resolved.opened.push(Media::File(path));
            }
        } else {
            self.resolved.skipped = self.others.len();
        }
        self.resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    fn names(items: &[Media]) -> Vec<String> {
        items.iter().map(Media::display_name).collect()
    }

    fn resolve_all(items: Vec<Media>) -> Resolved {
        resolve(items, &Cancel::default()).expect("not cancelled")
    }

    #[test]
    fn mixed_drop_plays_media_attaches_subtitles_and_skips_the_rest() {
        let dir = scratch_dir();
        for name in ["movie.mkv", "movie.srt", "readme.txt", "setup.exe", "list.m3u"] {
            fs::write(dir.join(name), b"plain bytes").unwrap();
        }
        fs::write(dir.join("list.m3u"), "a.mp3\nb.mp3\n").unwrap();
        let items = ["movie.mkv", "movie.srt", "readme.txt", "setup.exe", "list.m3u"]
            .map(|name| Media::File(dir.join(name)));
        let resolved = resolve_all(items.to_vec());
        assert_eq!(names(&resolved.media), ["movie.mkv", "a.mp3", "b.mp3"]);
        assert_eq!(resolved.subtitles, [dir.join("movie.srt")]);
        assert_eq!(resolved.skipped, 2);
        assert_eq!(names(&resolved.opened), ["movie.mkv", "list.m3u"]);
        assert_eq!(resolved.problems.len(), 0);
    }

    #[test]
    fn unrecognized_files_alone_go_to_the_engine() {
        let dir = scratch_dir();
        fs::write(dir.join("raw.h264"), b"\x00\x00\x00\x01gB").unwrap();
        let resolved = resolve_all(vec![Media::File(dir.join("raw.h264"))]);
        assert_eq!(names(&resolved.media), ["raw.h264"]);
        assert_eq!(resolved.skipped, 0);
    }

    #[test]
    fn folders_urls_and_problems() {
        let dir = scratch_dir();
        fs::create_dir(dir.join("empty")).unwrap();
        fs::create_dir(dir.join("full")).unwrap();
        fs::write(dir.join("full/b.mp4"), b"x").unwrap();
        fs::write(dir.join("empty.pls"), "[playlist]\n").unwrap();
        let resolved = resolve_all(vec![
            Media::File(dir.join("full")),
            Media::Url("https://x/live".into()),
            Media::File(dir.join("empty")),
            Media::File(dir.join("empty.pls")),
        ]);
        assert_eq!(names(&resolved.media), ["b.mp4", "https://x/live"]);
        let problems: Vec<Problem> = resolved.problems.iter().map(|(_, p)| *p).collect();
        assert_eq!(problems, [Problem::NoMediaInFolder, Problem::EmptyPlaylist]);
    }

    #[test]
    fn cancelled_resolves_to_nothing() {
        let cancel = Cancel::default();
        cancel.cancel();
        assert!(resolve(vec![Media::Url("https://x".into())], &cancel).is_none());
    }
}
