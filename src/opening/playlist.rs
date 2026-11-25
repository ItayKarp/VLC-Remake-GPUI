//! Playlist files (S1.3.4): `.m3u`, `.m3u8`, `.pls`, `.xspf`, `.cue` and `.asx` expand into
//! the items they list instead of being played.
//!
//! A local playlist that lists another local playlist is expanded too, down to
//! [`MAX_DEPTH`] levels; one that lists itself, directly or through others, is expanded once
//! (A includes B includes A). Playlists on the network (`http://…/list.m3u`) stay single items:
//! libVLC fetches and expands them itself. Entries are not checked for existence here; a missing
//! file fails when its turn comes. Links that aren't media and cameras or the screen are left out
//! (S9.7.2): a downloaded playlist must not reach libVLC's other access modules or start a
//! capture.

mod lines;
mod text;
mod xml;

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use super::Cancel;
use super::kind::{self, Kind, PlaylistFormat};
use crate::app::media::Media;
use text::Entry;

/// How deep playlists may nest inside each other.
pub const MAX_DEPTH: usize = 8;
/// The largest playlist file read; anything bigger isn't a playlist anyone wrote by hand.
const MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;
/// The most items one expansion produces, however the playlists nest.
const MAX_ITEMS: usize = 100_000;
/// The longest one expansion runs: past it, what was read so far opens. Many nested playlists
/// on a slow network drive must not keep "Opening …" up forever.
const MAX_TIME: Duration = Duration::from_secs(10);

/// What a playlist file turned out to hold.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Parsed {
    /// Its entries, in order.
    Entries(Vec<Entry>),
    /// It is a stream to play, not a list (a local HLS `.m3u8`).
    Stream,
}

/// Why a playlist couldn't be expanded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaylistError {
    /// The file couldn't be read.
    Unreadable(io::ErrorKind),
    /// The file is larger than any playlist should be.
    TooLarge,
    /// The playlist lists nothing.
    Empty,
}

/// Reads `text` as a playlist in `format`.
pub fn parse(format: PlaylistFormat, text: &str) -> Parsed {
    match format {
        PlaylistFormat::M3u => lines::m3u(text),
        PlaylistFormat::Pls => lines::pls(text),
        PlaylistFormat::Cue => lines::cue(text),
        PlaylistFormat::Xspf => xml::xspf(text),
        PlaylistFormat::Asx => xml::asx(text),
    }
}

/// The items the playlist at `path` lists, with nested local playlists expanded. A playlist
/// that is really a stream comes back as itself.
pub fn expand(
    path: &Path,
    format: PlaylistFormat,
    cancel: &Cancel,
) -> Result<Vec<Media>, PlaylistError> {
    let deadline = Instant::now() + MAX_TIME;
    let mut expander = Expander { visited: HashSet::new(), items: Vec::new(), cancel, deadline };
    expander.expand(path, format, 0)?;
    if expander.items.is_empty() {
        return Err(PlaylistError::Empty);
    }
    Ok(expander.items)
}

/// Whether a playlist file may list `item` (S9.7.2); logs one it may not.
fn may_be_listed(item: &Media) -> bool {
    let listed = item.may_be_listed();
    if !listed {
        tracing::warn!("playlist: skipped an entry that isn't media or is a capture");
    }
    listed
}

struct Expander<'a> {
    /// The playlists already expanded, by canonical path.
    visited: HashSet<PathBuf>,
    items: Vec<Media>,
    cancel: &'a Cancel,
    /// When to stop reading and keep what was read.
    deadline: Instant,
}

impl Expander<'_> {
    fn expand(
        &mut self,
        path: &Path,
        format: PlaylistFormat,
        depth: usize,
    ) -> Result<(), PlaylistError> {
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if !self.visited.insert(canonical) {
            tracing::warn!("playlist: {} includes itself; skipped", crate::logging::path(path));
            return Ok(());
        }
        let entries = match parse(format, &read(path)?) {
            Parsed::Stream => {
                self.items.push(Media::File(path.to_path_buf()));
                return Ok(());
            }
            Parsed::Entries(entries) => entries,
        };
        let items = entries.iter().filter_map(|entry| text::resolve(entry, path));
        for item in items.filter(may_be_listed) {
            if self.should_stop() {
                break;
            }
            self.add(item, depth);
        }
        Ok(())
    }

    /// Whether to stop: cancelled, or past the item or time limit (logged).
    fn should_stop(&self) -> bool {
        if self.cancel.is_cancelled() {
            return true;
        }
        let full = self.items.len() >= MAX_ITEMS;
        let late = Instant::now() >= self.deadline;
        if full || late {
            tracing::warn!(
                "playlist: stopped at {} items (full: {full}, late: {late})",
                self.items.len()
            );
        }
        full || late
    }

    /// Adds `item`, expanding it first if it is a local playlist and the nesting allows.
    /// A nested playlist that can't be read is skipped; the rest of the list still plays.
    fn add(&mut self, item: Media, depth: usize) {
        let nested = item.as_path().and_then(kind::by_extension);
        let (Some(Kind::Playlist(format)), Media::File(path)) = (nested, &item) else {
            self.items.push(item);
            return;
        };
        if depth + 1 >= MAX_DEPTH {
            tracing::warn!("playlist: nested deeper than {MAX_DEPTH}; skipped");
            return;
        }
        self.expand_nested(path, format, depth + 1);
    }

    /// Expands the nested playlist at `path`, `depth` levels down; skipped (logged) when it
    /// can't be read.
    fn expand_nested(&mut self, path: &Path, format: PlaylistFormat, depth: usize) {
        if let Err(error) = self.expand(path, format, depth) {
            tracing::warn!("playlist: skipped {}: {error:?}", crate::logging::path(path));
        }
    }
}

/// The playlist's text, whatever its encoding.
fn read(path: &Path) -> Result<String, PlaylistError> {
    let unreadable = |error: io::Error| PlaylistError::Unreadable(error.kind());
    if fs::metadata(path).map_err(unreadable)?.len() > MAX_FILE_SIZE {
        return Err(PlaylistError::TooLarge);
    }
    Ok(text::decode(&fs::read(path).map_err(unreadable)?))
}

#[cfg(test)]
mod tests;
