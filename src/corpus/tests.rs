//! The app's own readers over the corpus (S9.8.1): subtitle encodings, playlists, file kinds,
//! odd names and the subtitles found beside them. libVLC's side is
//! `engine::libvlc::corpus_tests`.

use std::collections::BTreeSet;

use super::{Corpus, CorpusFile, Expect, Kind, assert_none_failed};
use crate::app::media::Media;
use crate::engine::sidecar;
use crate::engine::subtitle_style::FileMatching;
use crate::engine::subtitle_text::{self, EncodingChoice};
use crate::opening::Cancel;
use crate::opening::kind::{self, Kind as OpenedKind};
use crate::opening::scratch::scratch_dir;
use crate::playlists::import;

#[test]
#[ignore = "needs the S9.8.1 corpus: tools/corpus/make.sh"]
fn subtitles_are_read_in_their_encoding() {
    let corpus = Corpus::load();
    let converted = scratch_dir();
    let failures: Vec<String> = corpus
        .files(Kind::Subtitle)
        .filter_map(|file| {
            let prepared =
                subtitle_text::prepare(&corpus.path(file), EncodingChoice::Guess, Some(&converted));
            let text = subtitle_text::decode(&prepared, "");
            let encoding = prepared.encoding.map(encoding_rs::Encoding::name);
            subtitle_problem(file, encoding, text.as_deref())
        })
        .collect();
    assert_none_failed("subtitle files", &failures);
}

/// What is wrong with how `file` was read, if anything.
fn subtitle_problem(
    file: &CorpusFile,
    encoding: Option<&str>,
    text: Option<&str>,
) -> Option<String> {
    let path = &file.path;
    if file.expect == Expect::Skipped {
        return text.map(|_| format!("{path}: read, but should be left alone"));
    }
    if let Some(wanted) = &file.encoding
        && encoding != Some(wanted.as_str())
    {
        return Some(format!("{path}: read as {encoding:?}, not {wanted}"));
    }
    let wanted = file.text.as_deref().unwrap_or_default();
    match text {
        Some(text) if text.contains(wanted) => None,
        Some(text) => Some(format!("{path}: no “{wanted}” in “{}”", first_line(text))),
        None => Some(format!("{path}: not read")),
    }
}

fn first_line(text: &str) -> String {
    text.lines().find(|line| line.chars().any(char::is_alphabetic)).unwrap_or("").to_owned()
}

#[test]
#[ignore = "needs the S9.8.1 corpus: tools/corpus/make.sh"]
fn playlists_import_with_their_items_and_missing_files() {
    let corpus = Corpus::load();
    let failures: Vec<String> = corpus
        .files(Kind::Playlist)
        .filter_map(|file| {
            let imported = import::read(&corpus.path(file), &Cancel::default());
            let path = &file.path;
            match (file.expect, imported) {
                (Expect::Fails, Ok(imported)) => {
                    Some(format!("{path}: imported {} items", imported.items.len()))
                }
                (Expect::Fails, Err(_)) => None,
                (_, Err(error)) => Some(format!("{path}: {error:?}")),
                (_, Ok(imported)) => {
                    let counts = (Some(imported.items.len()), Some(imported.missing));
                    (counts != (file.items, file.missing))
                        .then(|| format!("{path}: (items, missing) {counts:?}"))
                }
            }
        })
        .collect();
    assert_none_failed("playlists", &failures);
}

#[test]
#[ignore = "needs the S9.8.1 corpus: tools/corpus/make.sh"]
fn odd_and_wrong_names_still_open_as_media() {
    let corpus = Corpus::load();
    let failures: Vec<String> = corpus
        .files(Kind::Media)
        // Not video/: its picture is no media extension, and opens only on its own.
        .filter(|file| file.expect == Expect::Plays && !file.path.starts_with("video/"))
        .filter_map(|file| {
            let path = corpus.path(file);
            match kind::classify(&path) {
                Ok(OpenedKind::Media) => None,
                other => Some(format!("{}: classified as {other:?}", file.path)),
            }
        })
        .collect();
    assert_none_failed("media files", &failures);
}

#[test]
#[ignore = "needs the S9.8.1 corpus: tools/corpus/make.sh"]
fn odd_names_survive_storing_and_display() {
    let corpus = Corpus::load();
    let failures: Vec<String> = corpus
        .files(Kind::Media)
        .filter_map(|file| {
            let media = Media::File(corpus.path(file));
            let stored = media.to_stored();
            if Media::from_stored(&stored).as_ref() != Some(&media) {
                return Some(format!("{}: stored as {stored:?}, read back differently", file.path));
            }
            let shown = media.display_name();
            (shown.trim().is_empty() || shown.chars().any(char::is_control))
                .then(|| format!("{}: shown as {shown:?}", file.path))
        })
        .collect();
    assert_none_failed("names", &failures);
}

#[test]
#[ignore = "needs the S9.8.1 corpus: tools/corpus/make.sh"]
fn subtitles_beside_a_video_are_found() {
    let corpus = Corpus::load();
    let failures: Vec<String> = corpus
        .files(Kind::Media)
        .filter(|file| !file.subtitles.is_empty())
        .filter_map(|file| {
            let found: BTreeSet<String> =
                sidecar::find(&corpus.path(file), FileMatching::default())
                    .into_iter()
                    .filter_map(|sidecar| {
                        Some(sidecar.path.file_name()?.to_string_lossy().into_owned())
                    })
                    .collect();
            let wanted: BTreeSet<String> = file.subtitles.iter().cloned().collect();
            (found != wanted).then(|| format!("{}: found {found:?}", file.path))
        })
        .collect();
    assert_none_failed("videos with subtitles", &failures);
}
