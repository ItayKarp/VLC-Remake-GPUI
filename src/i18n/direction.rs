//! Which way the interface runs (S9.2.2): left to right, or mirrored for right-to-left
//! languages. See `docs/adr/0004-right-to-left-layout.md` for what mirrors and what doesn't.

use super::active;

/// The direction text is written in, and so the order a row's items go in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Direction {
    /// Left to right: the start is the left.
    #[default]
    Ltr,
    /// Right to left: the start is the right, and rows are mirrored.
    Rtl,
}

impl Direction {
    /// Whether this is right to left.
    pub fn is_rtl(self) -> bool {
        self == Self::Rtl
    }
}

/// The interface language's direction.
pub fn direction() -> Direction {
    active::get().language.direction()
}
