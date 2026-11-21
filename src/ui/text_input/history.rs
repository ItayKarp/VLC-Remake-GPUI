//! Undo and redo history for [`super::EditState`].
//!
//! Runs of typing or deleting made in quick succession merge into a single undo step, so undo
//! takes back a word rather than a letter.

use std::ops::Range;
use std::time::{Duration, Instant};

/// Edits typed within this long of each other undo together.
const UNDO_COALESCE: Duration = Duration::from_millis(800);

/// The most undo steps kept; older ones are dropped first.
const UNDO_LIMIT: usize = 100;

/// The text and selection at one point in time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    /// The whole text.
    pub(super) text: String,
    /// The selection, as UTF-8 byte offsets.
    pub(super) selection: Range<usize>,
}

/// What an edit did, which decides whether it merges with the one before it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditKind {
    /// Inserting a single character with nothing selected.
    Typing,
    /// Removing text without inserting any.
    Deleting,
    /// Anything else: pastes, replacements and IME compositions never merge.
    Other,
}

/// The undo and redo stacks plus the last edit, for coalescing.
#[derive(Clone, Debug, Default)]
pub(super) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last_edit: Option<(EditKind, Instant)>,
}

impl History {
    /// Forgets every step, as when the text is replaced from outside.
    pub(super) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.last_edit = None;
    }

    /// Ends the current run of typing, so the next edit starts a new undo step.
    pub(super) fn break_run(&mut self) {
        self.last_edit = None;
    }

    /// Records an undo step for the state before an edit, merging runs of typing or deleting.
    pub(super) fn checkpoint(
        &mut self,
        kind: EditKind,
        now: Instant,
        text: &str,
        selection: &Range<usize>,
    ) {
        let merge = matches!(
            self.last_edit,
            Some((last, at))
                if last == kind && kind != EditKind::Other
                    && now.duration_since(at) < UNDO_COALESCE
        );
        if !merge {
            self.undo.push(Snapshot { text: text.to_owned(), selection: selection.clone() });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = Some((kind, now));
    }

    /// Notes that an IME composition was committed. Its start was recorded when the composition
    /// began, so the whole composition undoes as one step and no new step is pushed here.
    pub(super) fn commit_composition(&mut self, now: Instant) {
        self.redo.clear();
        self.last_edit = Some((EditKind::Other, now));
    }

    /// Takes the most recent undo step, if any.
    pub(super) fn pop_undo(&mut self) -> Option<Snapshot> {
        self.undo.pop()
    }

    /// Takes the most recent redo step, if any.
    pub(super) fn pop_redo(&mut self) -> Option<Snapshot> {
        self.redo.pop()
    }

    /// Saves the state an undo replaced, so redo can bring it back.
    pub(super) fn push_redo(&mut self, snapshot: Snapshot) {
        self.redo.push(snapshot);
    }

    /// Saves the state a redo replaced, so undo can bring it back.
    pub(super) fn push_undo(&mut self, snapshot: Snapshot) {
        self.undo.push(snapshot);
    }
}
