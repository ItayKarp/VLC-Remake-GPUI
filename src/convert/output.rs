//! Where a conversion writes (S8.10.1): next to its source or in a folder the user picked,
//! named after the source with the preset's extension. A name that is taken (a file there, the
//! source itself, or another conversion's output) gets a number, so nothing is written over.

use std::path::{Path, PathBuf};

use super::Preset;
use crate::app::media::Media;

/// The name a conversion gets when its source names nothing usable.
const FALLBACK_NAME: &str = "Converted";
/// Characters file systems refuse in names.
const UNSAFE: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// Where conversions go.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Destination {
    /// The source's folder; streams and discs, which have none, go to the Videos or Music
    /// folder.
    #[default]
    NextToSource,
    /// A folder the user picked.
    Folder(PathBuf),
}

/// The file converting `source` to `preset` writes to under `destination`: the first name that
/// `taken` says is free.
pub fn plan(
    source: &Media,
    preset: Preset,
    destination: &Destination,
    taken: impl Fn(&Path) -> bool,
) -> PathBuf {
    let folder = folder(source, preset, destination);
    let stem = stem(source);
    let extension = preset.extension();
    let mut path = folder.join(format!("{stem}.{extension}"));
    let mut number = 1;
    while taken(&path) || is_source(source, &path) {
        number += 1;
        path = folder.join(format!("{stem} ({number}).{extension}"));
    }
    path
}

/// Whether `output` is `source`'s own file, which a conversion must never write over.
pub fn is_source(source: &Media, output: &Path) -> bool {
    source.as_path().is_some_and(|path| same_file(path, output))
}

/// The folder a conversion of `source` goes to.
fn folder(source: &Media, preset: Preset, destination: &Destination) -> PathBuf {
    match (destination, source.as_path().and_then(Path::parent)) {
        (Destination::Folder(folder), _) => folder.clone(),
        (Destination::NextToSource, Some(parent)) => parent.to_path_buf(),
        (Destination::NextToSource, None) if preset.is_audio_only() => {
            dirs::audio_dir().unwrap_or_else(crate::recording::default_folder)
        }
        (Destination::NextToSource, None) => crate::recording::default_folder(),
    }
}

/// The name a conversion of `source` starts from: the file's own, else what the source is
/// called, without characters file systems refuse.
fn stem(source: &Media) -> String {
    let name = match source.as_path() {
        Some(path) => path.file_stem().map(|stem| stem.to_string_lossy().into_owned()),
        None => Some(source.display_name()),
    };
    let cleaned: String = name
        .unwrap_or_default()
        .chars()
        .map(|c| if UNSAFE.contains(&c) || c.is_control() { '_' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.');
    if cleaned.is_empty() { FALLBACK_NAME.to_owned() } else { cleaned.to_owned() }
}

/// Whether `a` and `b` name one file: the same path, or, on file systems that ignore case
/// (macOS's and Windows' by default), the same folder and a name differing only in case.
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    let folded = |path: &Path| path.to_string_lossy().to_lowercase();
    cfg!(any(target_os = "macos", target_os = "windows")) && folded(a) == folded(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> Media {
        Media::File(PathBuf::from(path))
    }

    #[test]
    fn a_file_converts_next_to_itself_with_the_preset_extension() {
        let output =
            plan(&file("/films/Holiday.mkv"), Preset::Mp4H264, &Destination::default(), |_| false);
        assert_eq!(output, Path::new("/films/Holiday.mp4"));
    }

    #[test]
    fn taken_names_and_the_source_itself_get_a_number() {
        let source = file("/films/Holiday.mp4");
        let taken = |path: &Path| path == Path::new("/films/Holiday (2).mp4");
        let output = plan(&source, Preset::Mp4H264, &Destination::default(), taken);
        assert_eq!(output, Path::new("/films/Holiday (3).mp4"));
    }

    #[test]
    fn a_picked_folder_takes_every_conversion() {
        let picked = Destination::Folder(PathBuf::from("/out"));
        let output = plan(&file("/films/Holiday.mkv"), Preset::Mp3, &picked, |_| false);
        assert_eq!(output, Path::new("/out/Holiday.mp3"));
    }

    #[test]
    fn streams_are_named_without_unsafe_characters() {
        let stream = Media::Url("https://example.com/live/a:b.m3u8".into());
        let picked = Destination::Folder(PathBuf::from("/out"));
        let output = plan(&stream, Preset::Mp4H264, &picked, |_| false);
        let name = output.file_name().expect("a name").to_string_lossy().into_owned();
        assert_eq!(output.extension(), Some("mp4".as_ref()));
        assert!(!name.contains(':'), "{name}");
    }

    #[test]
    fn the_source_is_recognised() {
        assert!(is_source(&file("/a/b.mp4"), Path::new("/a/b.mp4")));
        assert!(!is_source(&file("/a/b.mp4"), Path::new("/a/b (2).mp4")));
        assert!(!is_source(&Media::Url("https://x".into()), Path::new("/a/b.mp4")));
    }
}
