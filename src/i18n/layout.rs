//! Layout that follows the interface's direction (S9.2.2). GPUI lays out left to right only (it
//! doesn't pass a direction to Taffy, whose flexbox ignores one anyway), so mirrored views say
//! "start" and "end" where they used to say left and right, and these pick the side.
//!
//! What mirrors and what doesn't is decided in `docs/adr/0004-right-to-left-layout.md`: the
//! player's transport, its seek bar and time codes, and the video stay left to right.

use gpui::{DefiniteLength, Length, Styled};

use super::direction::{Direction, direction};

/// Start/end variants of GPUI's left/right styles, mirrored for right-to-left languages.
pub trait DirectionalStyle: Styled + Sized {
    /// A flex row whose first child is at the start: the left, or the right when mirrored.
    fn flex_row_start(self) -> Self {
        flex_row_in(self, direction())
    }

    /// Padding on the start side.
    fn ps(self, length: impl Into<DefiniteLength> + Clone) -> Self {
        if direction().is_rtl() { self.pr(length) } else { self.pl(length) }
    }

    /// Padding on the end side.
    fn pe(self, length: impl Into<DefiniteLength> + Clone) -> Self {
        if direction().is_rtl() { self.pl(length) } else { self.pr(length) }
    }

    /// Margin on the start side.
    fn ms(self, length: impl Into<Length> + Clone) -> Self {
        if direction().is_rtl() { self.mr(length) } else { self.ml(length) }
    }

    /// Margin on the end side.
    fn me(self, length: impl Into<Length> + Clone) -> Self {
        if direction().is_rtl() { self.ml(length) } else { self.mr(length) }
    }

    /// An absolute position's distance from the start edge.
    fn inset_start(self, length: impl Into<Length> + Clone) -> Self {
        if direction().is_rtl() { self.right(length) } else { self.left(length) }
    }

    /// An absolute position's distance from the end edge.
    fn inset_end(self, length: impl Into<Length> + Clone) -> Self {
        if direction().is_rtl() { self.left(length) } else { self.right(length) }
    }

    /// Text aligned to the start side.
    fn text_start(self) -> Self {
        if direction().is_rtl() { self.text_right() } else { self.text_left() }
    }
}

impl<T: Styled> DirectionalStyle for T {}

/// `ltr`, or `rtl` when the interface is mirrored: an icon that points along the reading
/// direction (back, forward, a submenu's chevron) passes its mirror image as `rtl`.
pub fn mirrored<T>(ltr: T, rtl: T) -> T {
    if direction().is_rtl() { rtl } else { ltr }
}

/// A flex row laid out in `direction`.
fn flex_row_in<T: Styled>(element: T, direction: Direction) -> T {
    match direction {
        Direction::Ltr => element.flex().flex_row(),
        Direction::Rtl => element.flex().flex_row_reverse(),
    }
}

#[cfg(test)]
mod tests {
    use gpui::{FlexDirection, StyleRefinement, Styled};

    use super::*;

    struct Probe(StyleRefinement);

    impl Styled for Probe {
        fn style(&mut self) -> &mut StyleRefinement {
            &mut self.0
        }
    }

    #[test]
    fn rows_reverse_only_right_to_left() {
        let ltr = flex_row_in(Probe(StyleRefinement::default()), Direction::Ltr);
        let rtl = flex_row_in(Probe(StyleRefinement::default()), Direction::Rtl);
        assert_eq!(ltr.0.flex_direction, Some(FlexDirection::Row));
        assert_eq!(rtl.0.flex_direction, Some(FlexDirection::RowReverse));
    }
}
