//! The test media corpus (S9.8.1) as tests see it: its manifest, which names each file, its
//! kind and what the app should do with it. The corpus lives outside the repository:
//! `tools/corpus/make.sh` makes it in `target/corpus`, `tools/corpus/fetch.sh` brings back the
//! pinned archive, and `RVLC_CORPUS_DIR` names another folder (`docs/qa.md`).
//!
//! Tests that read it are ignored by default; run them with
//! `cargo test corpus -- --ignored --test-threads 1`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Names the corpus folder when it isn't `target/corpus`.
const DIR_VAR: &str = "RVLC_CORPUS_DIR";
/// The folder of names Windows refuses, which `fetch.sh` leaves out there.
const UNIX_NAMES: &str = "names-unix/";

/// What a file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Audio, video or a picture, for the engine.
    Media,
    /// A subtitle file.
    Subtitle,
    /// A playlist file.
    Playlist,
}

/// What the app should do with a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Expect {
    /// Media opens and plays.
    Plays,
    /// Media fails to open, or a playlist can't be read.
    Fails,
    /// A text file is read: a subtitle's text, a playlist's items.
    Reads,
    /// The app leaves the file alone (a subtitle it doesn't read itself).
    Skipped,
}

/// One file of the corpus and what tests check about it.
#[derive(Clone, Debug, Deserialize)]
pub struct CorpusFile {
    /// Where it is, relative to the corpus folder, with `/` between folders.
    pub path: String,
    /// What it is.
    pub kind: Kind,
    /// What the app should do with it.
    pub expect: Expect,
    /// A media file's length.
    pub seconds: Option<u64>,
    /// A subtitle's encoding, as `encoding_rs` names it.
    pub encoding: Option<String>,
    /// A line of a subtitle's text.
    pub text: Option<String>,
    /// A playlist's item count.
    pub items: Option<usize>,
    /// How many of a playlist's items are missing files.
    pub missing: Option<usize>,
    /// The subtitle files found beside a video, by name.
    #[serde(default)]
    pub subtitles: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    file: Vec<CorpusFile>,
}

/// The corpus: its folder and the files its manifest lists.
pub struct Corpus {
    root: PathBuf,
    files: Vec<CorpusFile>,
}

impl Corpus {
    /// The corpus in `RVLC_CORPUS_DIR` or `target/corpus`. Panics, saying how to make one,
    /// when there is none.
    pub fn load() -> Self {
        let root = std::env::var_os(DIR_VAR).map_or_else(
            || Path::new(env!("CARGO_MANIFEST_DIR")).join("target/corpus"),
            PathBuf::from,
        );
        let manifest = root.join("manifest.toml");
        let text = std::fs::read_to_string(&manifest).unwrap_or_else(|error| {
            panic!("no corpus at {} ({error}): run tools/corpus/make.sh", root.display())
        });
        let manifest: Manifest = toml::from_str(&text).expect("the corpus manifest parses");
        assert_eq!(manifest.version, 1, "a corpus made by another version of make.sh");
        let unix_names = root.join(UNIX_NAMES).is_dir();
        let files = manifest
            .file
            .into_iter()
            .filter(|file| unix_names || !file.path.starts_with(UNIX_NAMES))
            .collect();
        Self { root, files }
    }

    /// The files of `kind`.
    pub fn files(&self, kind: Kind) -> impl Iterator<Item = &CorpusFile> {
        self.files.iter().filter(move |file| file.kind == kind)
    }

    /// Where `file` is on disk.
    pub fn path(&self, file: &CorpusFile) -> PathBuf {
        file.path.split('/').fold(self.root.clone(), |path, part| path.join(part))
    }
}

/// Panics listing every failure in `failures`, so one run shows them all.
pub fn assert_none_failed(what: &str, failures: &[String]) {
    assert!(failures.is_empty(), "{} {what} failed:\n{}", failures.len(), failures.join("\n"));
}

#[cfg(test)]
mod tests;
