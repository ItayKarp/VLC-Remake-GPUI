//! Moving a unit one place at a time, for Customize Player's list (keyboard-only editing,
//! S8.7): past the next item, spacer or divider of its bar, the designed group gaps skipped as
//! they don't move, and from a bar's end into the next bar in list order.

use super::{Spot, Target};
use crate::player_layout::{Layout, Piece, Zone};

/// Which way a unit moves in the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Up the list: left in a bar, or to the end of the bar before.
    Earlier,
    /// Down the list: right in a bar, or to the start of the bar after.
    Later,
}

impl Layout {
    /// The bars in list order: the title bar, its window controls, then the control bar's rows
    /// from the top.
    pub fn zones(&self) -> Vec<Zone> {
        let rows = (0..self.rows.len()).map(Zone::Row);
        [Zone::TitleBar, Zone::WindowControls].into_iter().chain(rows).collect()
    }

    /// Moves the unit at `from` one place `step`; where it lands, or none when it can't move
    /// (it stays put, or there's nowhere further to go).
    pub fn nudge(&mut self, from: Spot, step: Step) -> Option<Spot> {
        let (_, range) = self.unit_at(from)?;
        let pieces = self.pieces(from.zone);
        let shown = |index: &usize| !matches!(pieces[*index], Piece::Gap(_));
        // The whole unit beside it: the timeline is three pieces.
        let span = |index: usize| self.unit_at(Spot { zone: from.zone, index });
        let to = match step {
            Step::Earlier => (0..range.start).rev().find(shown).map(|index| {
                let start = span(index).map_or(index, |(_, unit)| unit.start);
                Spot { zone: from.zone, index: start }
            }),
            Step::Later => (range.end..pieces.len()).find(shown).map(|index| {
                let end = span(index).map_or(index + 1, |(_, unit)| unit.end);
                Spot { zone: from.zone, index: end }
            }),
        };
        let to = match to {
            Some(to) => to,
            None => self.next_zone(from.zone, step)?,
        };
        let rows = self.rows.len();
        self.move_unit(from, Target::Into(to));
        Some(self.landed(from, to, range.len(), rows))
    }

    /// The end of the bar before `zone`, or the start of the bar after it.
    fn next_zone(&self, zone: Zone, step: Step) -> Option<Spot> {
        let zones = self.zones();
        let at = zones.iter().position(|known| *known == zone)?;
        let next = match step {
            Step::Earlier => *zones.get(at.checked_sub(1)?)?,
            Step::Later => *zones.get(at + 1)?,
        };
        let index = if step == Step::Earlier { self.pieces(next).len() } else { 0 };
        Some(Spot { zone: next, index })
    }

    /// Where a unit of `len` pieces moved from `from` to `to` is now, `rows` being how many
    /// rows there were before: a row it left empty may have gone.
    fn landed(&self, from: Spot, to: Spot, len: usize, rows: usize) -> Spot {
        let index =
            if to.zone == from.zone && to.index > from.index { to.index - len } else { to.index };
        let zone = match (from.zone, to.zone) {
            (Zone::Row(left), Zone::Row(row)) if self.rows.len() < rows && row > left => {
                Zone::Row(row - 1)
            }
            _ => to.zone,
        };
        Spot { zone, index }
    }
}
