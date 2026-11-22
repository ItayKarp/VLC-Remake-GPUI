//! The listing's selection (S4.6.5, S4.6.6): a click selects one item, Ctrl+click (Cmd+click on
//! macOS) adds or removes one, Shift+click selects the range from the last plain click, and
//! Ctrl/Cmd+A selects everything. The arrow keys, Home, End and Page Up and Down move it (Shift
//! extends it).
//!
//! Items are kept by what they are rather than where they are, so a reload that adds rows above
//! them, or a file renamed while selected (its media stays the same), keeps the selection.

use std::path::PathBuf;

use crate::app::media::Media;
use crate::db::media::MediaId;
use crate::navigation::route::Route;
use crate::ui::focus::{self, Layout, Move};

/// What a listed item is, which outlives its place in the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Key {
    /// The "+ Add" row.
    Add,
    /// A row that goes somewhere: a folder, a category, a playlist.
    Place(Route),
    /// A row that plays media it names (History).
    Recent(Media),
    /// A card of media the library identified.
    Media(MediaId),
    /// A card of a file the library hasn't identified (yet, or it is outside the library).
    File(PathBuf),
}

/// The selected items, where ranges grow from, and the item the keyboard is on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Selection {
    keys: Vec<Key>,
    /// Where Shift+click and Shift+arrow ranges start.
    anchor: Option<Key>,
    /// The item the arrow keys move from and Enter opens: the last one clicked or reached.
    cursor: Option<Key>,
}

impl Selection {
    /// Where the keyboard's item is among `keys`, if it is still listed.
    pub(super) fn cursor(&self, keys: &[Key]) -> Option<usize> {
        position(keys, self.cursor.as_ref()?)
    }

    /// Whether `key` is selected.
    pub(super) fn contains(&self, key: &Key) -> bool {
        self.keys.contains(key)
    }

    /// The indices of the selected items among `keys`, in list order.
    pub(super) fn indices(&self, keys: &[Key]) -> Vec<usize> {
        (0..keys.len()).filter(|&index| self.contains(&keys[index])).collect()
    }

    /// Selects only `key` (a plain click).
    pub(super) fn select(&mut self, key: Key) {
        self.keys = vec![key.clone()];
        self.anchor = Some(key.clone());
        self.cursor = Some(key);
    }

    /// Adds `key` to the selection, or takes it out (Ctrl/Cmd+click).
    pub(super) fn toggle(&mut self, key: Key) {
        match self.keys.iter().position(|selected| *selected == key) {
            Some(at) => drop(self.keys.remove(at)),
            None => self.keys.push(key.clone()),
        }
        self.anchor = Some(key.clone());
        self.cursor = Some(key);
    }

    /// Selects the items from the anchor to `keys[end]` (Shift+click).
    pub(super) fn extend_to(&mut self, keys: &[Key], end: usize) {
        let Some(key) = keys.get(end) else { return };
        let start = self.anchor.as_ref().and_then(|anchor| position(keys, anchor)).unwrap_or(end);
        let (low, high) = (start.min(end), start.max(end));
        self.keys = keys[low..=high].to_vec();
        self.anchor = Some(keys[start].clone());
        self.cursor = Some(key.clone());
    }

    /// Selects every item (Ctrl/Cmd+A).
    pub(super) fn select_all(&mut self, keys: &[Key]) {
        self.keys = keys.to_vec();
    }

    /// Forgets the selection (another place is shown).
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Moves the keyboard as `movement` says among `keys`, laid out as `layout` says (its
    /// columns and its page), selecting the item it lands on or, with `extend`, the range from
    /// the anchor to it. Returns the item's index. With no item, the first move lands on an end.
    pub(super) fn step(
        &mut self,
        keys: &[Key],
        movement: Move,
        layout: Layout,
        extend: bool,
    ) -> Option<usize> {
        let next = focus::step(self.cursor(keys), keys.len(), movement, layout, |_| true)?;
        if extend {
            self.extend_to(keys, next);
        } else {
            self.select(keys[next].clone());
        }
        Some(next)
    }
}

fn position(keys: &[Key], key: &Key) -> Option<usize> {
    keys.iter().position(|listed| listed == key)
}

#[cfg(test)]
mod tests;
