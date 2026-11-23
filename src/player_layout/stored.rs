//! Reading a [`Layout`] from its stored ids and writing it back (S8.7.5).
//!
//! Each item shows once: a second copy of an id is dropped. Ids this build doesn't know are
//! skipped and remembered with their place, so saving the layout again keeps them (a layout
//! from a newer version). Rows past [`MAX_ROWS`] are ignored.
//!
//! Minimize, Maximize and Close are always among the window controls: found anywhere else they
//! are dropped, and missing they are put back. A layout saved before the window controls could
//! be customized has none stored, and gets the designed ones.

use std::collections::HashSet;

use super::{Item, Layout, MAX_ROWS, Piece, Unknown, Zone};
use crate::settings::StoredLayout;

/// The stored id of the time readouts and seek bar, which move as one.
const TIMELINE: &str = "timeline";
const SPACER: &str = "spacer";
const DIVIDER: &str = "divider";
/// A designed group gap's id is this followed by its width: `gap-16`.
const GAP_PREFIX: &str = "gap-";
/// The widest a stored gap may be, so a hand-edited file can't push the bar off the window.
const MAX_GAP: f32 = 64.;

/// Every item with an id of its own; the timeline's three are stored as [`TIMELINE`].
const IDS: &[(Item, &str)] = &[
    (Item::Menu, "menu"),
    (Item::Sidebar, "sidebar"),
    (Item::Folder, "folder"),
    (Item::Title, "title"),
    (Item::Previous, "previous"),
    (Item::Back15, "back-15"),
    (Item::PlayPause, "play-pause"),
    (Item::Forward15, "forward-15"),
    (Item::Next, "next"),
    (Item::Volume, "volume"),
    (Item::Audio, "audio"),
    (Item::Subtitles, "subtitles"),
    (Item::Settings, "playback-settings"),
    (Item::List, "queue"),
    (Item::Back10, "back-10"),
    (Item::Forward10, "forward-10"),
    (Item::Speed, "speed"),
    (Item::Stop, "stop"),
    (Item::Repeat, "repeat"),
    (Item::Shuffle, "shuffle"),
    (Item::Record, "record"),
    (Item::Snapshot, "snapshot"),
    (Item::PictureInPicture, "picture-in-picture"),
    (Item::Quit, "quit"),
    (Item::VlcSettings, "vlc-settings"),
    (Item::GoHome, "go-home"),
    (Item::FileName, "file-name"),
    (Item::FolderName, "folder-name"),
    (Item::Properties, "properties"),
    (Item::Replay, "replay"),
    (Item::Chapters, "chapters"),
    (Item::Bookmark, "bookmark"),
    (Item::Bookmarks, "bookmarks"),
    (Item::Playlists, "playlists"),
    (Item::Favorite, "favorite"),
    (Item::Hdr, "hdr"),
    (Item::Zoom, "zoom"),
    (Item::RightSidebar, "right-sidebar"),
    (Item::Thumbnail, "thumbnail"),
    (Item::ArtistName, "artist-name"),
    (Item::PlaylistName, "playlist-name"),
    (Item::Minimize, "minimize"),
    (Item::Maximize, "maximize"),
    (Item::Fullscreen, "fullscreen"),
    (Item::Close, "close"),
];

/// The layout `stored` describes: the designed one when it has no rows.
pub(super) fn parse(stored: &StoredLayout) -> Layout {
    if stored.controls.is_empty() {
        return Layout::designed();
    }
    let mut reader = Reader::default();
    let title_bar = reader.zone(Zone::TitleBar, &stored.title_bar);
    let rows = stored.controls.iter().take(MAX_ROWS).enumerate();
    let rows = rows.map(|(row, ids)| reader.zone(Zone::Row(row), ids)).collect();
    let mut window_controls = reader.zone(Zone::WindowControls, &stored.window_controls);
    // Before they could be customized, all four stood in the title bar.
    let older = stored.window_controls.is_empty();
    for item in [Item::Minimize, Item::Maximize, Item::Fullscreen, Item::Close] {
        if (item.is_fixed() || older) && reader.seen.insert(item) {
            window_controls.push(Piece::Item(item));
        }
    }
    Layout { title_bar, window_controls, rows, unknown: reader.unknown }
}

/// `layout` as stored: nothing for the designed layout.
pub(super) fn write(layout: &Layout) -> StoredLayout {
    if *layout == Layout::designed() {
        return StoredLayout::default();
    }
    let zone = |zone: Zone, pieces: &[Piece]| ids(pieces, layout.unknown_in(zone));
    StoredLayout {
        title_bar: zone(Zone::TitleBar, &layout.title_bar),
        window_controls: zone(Zone::WindowControls, &layout.window_controls),
        controls: layout
            .rows
            .iter()
            .enumerate()
            .map(|(row, row_pieces)| zone(Zone::Row(row), row_pieces))
            .collect(),
    }
}

impl Layout {
    /// The unknown ids kept for `zone`, in their stored order.
    fn unknown_in(&self, zone: Zone) -> impl Iterator<Item = &Unknown> {
        self.unknown.iter().filter(move |unknown| unknown.zone == zone)
    }
}

/// The items seen so far, so each shows once, and the ids skipped.
#[derive(Default)]
struct Reader {
    seen: HashSet<Item>,
    unknown: Vec<Unknown>,
}

impl Reader {
    fn zone(&mut self, zone: Zone, ids: &[String]) -> Vec<Piece> {
        let mut pieces = Vec::new();
        for (index, id) in ids.iter().enumerate() {
            let Some(parsed) = pieces_of(id) else {
                self.unknown.push(Unknown { zone, index, id: id.clone() });
                continue;
            };
            pieces.extend(parsed.into_iter().filter(|piece| match piece {
                // The fixed window controls stand nowhere else.
                Piece::Item(item) if item.is_fixed() && zone != Zone::WindowControls => false,
                Piece::Item(item) => self.seen.insert(*item),
                _ => true,
            }));
        }
        pieces
    }
}

/// What the stored `id` stands for, if this build knows it.
fn pieces_of(id: &str) -> Option<Vec<Piece>> {
    let piece = match id {
        TIMELINE => {
            return Some(
                [Item::CurrentTime, Item::SeekBar, Item::Duration].map(Piece::Item).to_vec(),
            );
        }
        SPACER => Piece::Spacer,
        DIVIDER => Piece::Divider,
        _ => match id.strip_prefix(GAP_PREFIX) {
            Some(width) => Piece::Gap(gap_width(width)?),
            None => Piece::Item(IDS.iter().find(|(_, known)| *known == id)?.0),
        },
    };
    Some(vec![piece])
}

fn gap_width(width: &str) -> Option<f32> {
    width.parse().ok().filter(|width: &f32| (0. ..=MAX_GAP).contains(width) && *width > 0.)
}

/// The ids of `pieces`, with the `unknown` ones put back at their stored places.
fn ids<'a>(pieces: &[Piece], unknown: impl Iterator<Item = &'a Unknown>) -> Vec<String> {
    let mut ids: Vec<String> = pieces.iter().filter_map(|piece| id_of(*piece)).collect();
    for unknown in unknown {
        ids.insert(unknown.index.min(ids.len()), unknown.id.clone());
    }
    ids
}

/// `piece`'s stored id; none for the seek bar and the length, which the timeline's id covers.
fn id_of(piece: Piece) -> Option<String> {
    Some(match piece {
        Piece::Spacer => SPACER.to_owned(),
        Piece::Divider => DIVIDER.to_owned(),
        Piece::Gap(width) => format!("{GAP_PREFIX}{width}"),
        Piece::Item(Item::CurrentTime) => TIMELINE.to_owned(),
        Piece::Item(Item::SeekBar | Item::Duration) => return None,
        Piece::Item(item) => IDS.iter().find(|(known, _)| *known == item)?.1.to_owned(),
    })
}

#[cfg(test)]
mod tests;
