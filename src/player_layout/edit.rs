//! Changing a [`Layout`] as Customize Player does (S8.7.3): moving a unit within or between the
//! bars, adding one from the palette and removing one into it. A unit is an item, a spacer or a
//! divider; the timeline's three items are one unit. The designed group gaps aren't units: they
//! stay where they are.
//!
//! A row left with nothing but gaps goes, unless it is the only one; dropping on the place for
//! a new row adds one above the others, up to [`MAX_ROWS`].

use std::ops::Range;

use super::{Item, Layout, MAX_ROWS, Piece, Zone};

/// What can be dragged around: one item (the timeline as [`Item::CurrentTime`]), a spacer or a
/// divider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    /// An item; it shows once.
    Item(Item),
    /// A spacer; any number can show.
    Spacer,
    /// A divider; any number can show.
    Divider,
}

/// A place in one of the bars: the piece at `index` of `zone`, or the gap before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spot {
    /// The bar.
    pub zone: Zone,
    /// The index among the zone's pieces; its length is the end.
    pub index: usize,
}

/// Where a dragged unit is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Into a bar, before the piece at the spot's index.
    Into(Spot),
    /// Into a new control-bar row above the others.
    NewRow,
}

/// The palette's units, in the designed order (`player_customize_01.png`), then the designed
/// bars' items, which show there once removed. Spacer and Divider come first.
const PALETTE: &[Item] = &[
    Item::Speed,
    Item::Forward10,
    Item::Back10,
    Item::Forward15,
    Item::Back15,
    Item::Stop,
    Item::Repeat,
    Item::Shuffle,
    Item::Record,
    Item::Snapshot,
    Item::PictureInPicture,
    Item::Quit,
    Item::VlcSettings,
    Item::GoHome,
    Item::FileName,
    Item::FolderName,
    Item::Properties,
    Item::Replay,
    Item::Chapters,
    Item::Bookmark,
    Item::Bookmarks,
    Item::Playlists,
    Item::Menu,
    Item::Sidebar,
    Item::Folder,
    Item::Title,
    Item::Previous,
    Item::PlayPause,
    Item::Next,
    Item::CurrentTime,
    Item::Volume,
    Item::Audio,
    Item::Subtitles,
    Item::Settings,
    Item::List,
];

impl Unit {
    /// The pieces this unit is in a bar.
    pub fn pieces(self) -> Vec<Piece> {
        match self {
            Unit::Item(Item::CurrentTime | Item::SeekBar | Item::Duration) => {
                [Item::CurrentTime, Item::SeekBar, Item::Duration].map(Piece::Item).to_vec()
            }
            Unit::Item(item) => vec![Piece::Item(item)],
            Unit::Spacer => vec![Piece::Spacer],
            Unit::Divider => vec![Piece::Divider],
        }
    }
}

impl Layout {
    /// The pieces in `zone`; none for a row that doesn't exist.
    pub fn pieces(&self, zone: Zone) -> &[Piece] {
        match zone {
            Zone::TitleBar => &self.title_bar,
            Zone::Row(row) => self.rows.get(row).map_or(&[], Vec::as_slice),
        }
    }

    /// The units the palette offers: Spacer, Divider, and the items not in the bars.
    pub fn palette(&self) -> Vec<Unit> {
        let items = PALETTE.iter().filter(|item| !self.is_placed(**item)).map(|i| Unit::Item(*i));
        [Unit::Spacer, Unit::Divider].into_iter().chain(items).collect()
    }

    /// Whether `item` is in one of the bars.
    pub fn is_placed(&self, item: Item) -> bool {
        let mut zones = std::iter::once(&self.title_bar).chain(&self.rows);
        zones.any(|pieces| pieces.contains(&Piece::Item(item)))
    }

    /// The unit at `spot` and the pieces it covers; none for a gap or past the end.
    pub fn unit_at(&self, spot: Spot) -> Option<(Unit, Range<usize>)> {
        let pieces = self.pieces(spot.zone);
        let unit = match *pieces.get(spot.index)? {
            Piece::Item(Item::CurrentTime | Item::SeekBar | Item::Duration) => {
                let start = pieces.iter().position(|p| *p == Piece::Item(Item::CurrentTime))?;
                return Some((Unit::Item(Item::CurrentTime), start..start + 3));
            }
            Piece::Item(item) => Unit::Item(item),
            Piece::Spacer => Unit::Spacer,
            Piece::Divider => Unit::Divider,
            Piece::Gap(_) => return None,
        };
        Some((unit, spot.index..spot.index + 1))
    }

    /// Moves the unit at `from` to `to`.
    pub fn move_unit(&mut self, from: Spot, to: Target) {
        let Some((unit, range)) = self.unit_at(from) else { return };
        let to = match to {
            Target::Into(spot) if spot.zone == from.zone && spot.index > range.start => {
                if spot.index <= range.end {
                    return;
                }
                Target::Into(Spot { index: spot.index - range.len(), ..spot })
            }
            other => other,
        };
        self.pieces_mut(from.zone).drain(range);
        self.insert(unit, to);
        self.tidy();
    }

    /// Adds `unit` from the palette at `to`; an item already in the bars stays where it is.
    pub fn add(&mut self, unit: Unit, to: Target) {
        if let Unit::Item(item) = unit
            && self.is_placed(item)
        {
            return;
        }
        self.insert(unit, to);
        self.tidy();
    }

    /// Removes the unit at `from` into the palette.
    pub fn remove(&mut self, from: Spot) {
        let Some((_, range)) = self.unit_at(from) else { return };
        self.pieces_mut(from.zone).drain(range);
        self.tidy();
    }

    fn insert(&mut self, unit: Unit, to: Target) {
        let pieces = unit.pieces();
        match to {
            Target::NewRow if self.rows.len() < MAX_ROWS => {
                self.rows.insert(0, pieces);
                self.shift_unknown_rows(0, |row| row + 1);
            }
            Target::NewRow => {
                self.insert(unit, Target::Into(Spot { zone: Zone::Row(0), index: 0 }));
            }
            Target::Into(spot) => {
                let zone = self.pieces_mut(spot.zone);
                let index = spot.index.min(zone.len());
                zone.splice(index..index, pieces);
            }
        }
    }

    /// Drops the rows left with nothing but gaps, keeping one.
    fn tidy(&mut self) {
        let mut row = 0;
        while row < self.rows.len() {
            let empty = self.rows[row].iter().all(|piece| matches!(piece, Piece::Gap(_)));
            if empty && self.rows.len() > 1 {
                self.rows.remove(row);
                self.unknown.retain(|unknown| unknown.zone != Zone::Row(row));
                self.shift_unknown_rows(row, |later| later - 1);
            } else {
                row += 1;
            }
        }
    }

    /// Renumbers the kept unknown ids of rows `from` and below with `shift`.
    fn shift_unknown_rows(&mut self, from: usize, shift: impl Fn(usize) -> usize) {
        for unknown in &mut self.unknown {
            if let Zone::Row(row) = unknown.zone
                && row >= from
            {
                unknown.zone = Zone::Row(shift(row));
            }
        }
    }

    fn pieces_mut(&mut self, zone: Zone) -> &mut Vec<Piece> {
        match zone {
            Zone::TitleBar => &mut self.title_bar,
            Zone::Row(row) => {
                let last = self.rows.len().saturating_sub(1);
                &mut self.rows[row.min(last)]
            }
        }
    }
}

#[cfg(test)]
mod tests;
