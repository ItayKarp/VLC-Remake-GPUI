//! How the centered title and the folder chip share the title bar (S2.1 edge cases): the title
//! stays centered on the window and never reaches the left or right groups; when the title and
//! the chip don't both fit, the chip goes first.

/// Space kept between the title and the groups beside it.
const TITLE_GAP: f32 = 12.;

/// Widths in pixels of what the title bar holds.
pub struct Widths {
    /// The whole bar.
    pub window: f32,
    /// The left group without the folder chip.
    pub left: f32,
    /// The folder chip, when there's one to show.
    pub chip: Option<f32>,
    /// The caption buttons.
    pub right: f32,
    /// The title's natural width.
    pub title: f32,
}

/// Where things go.
#[derive(Debug, PartialEq)]
pub struct Fit {
    /// Whether the folder chip shows.
    pub chip: bool,
    /// The widest the title may be; past it, it's cut with an ellipsis.
    pub title_max: f32,
}

/// Shows the chip only if the whole title still fits beside it, and gives the title all the
/// room the wider of the two side groups leaves on both sides of the centre.
pub fn fit(widths: &Widths) -> Fit {
    let room = |left: f32| (widths.window - 2.0 * (left.max(widths.right) + TITLE_GAP)).max(0.0);
    let chip = widths.chip.filter(|chip| room(widths.left + chip) >= widths.title);
    Fit { chip: chip.is_some(), title_max: room(widths.left + chip.unwrap_or(0.0)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths(window: f32, title: f32) -> Widths {
        Widths { window, left: 120.0, chip: Some(80.0), right: 184.0, title }
    }

    #[test]
    fn a_short_title_leaves_the_chip() {
        let fit = fit(&widths(1280.0, 200.0));
        assert_eq!(fit, Fit { chip: true, title_max: 1280.0 - 2.0 * (200.0 + TITLE_GAP) });
    }

    #[test]
    fn a_long_title_collapses_the_chip_first() {
        // With the chip the left group is 200 wide, so the title gets 856; it needs 900.
        let fit = fit(&widths(1280.0, 900.0));
        assert!(!fit.chip);
        assert_eq!(fit.title_max, 1280.0 - 2.0 * (184.0 + TITLE_GAP));
    }

    #[test]
    fn a_title_too_long_even_alone_is_cut() {
        let fit = fit(&widths(800.0, 2000.0));
        assert!(!fit.chip);
        assert!(fit.title_max < 2000.0);
    }

    #[test]
    fn no_chip_to_show_is_not_shown() {
        let fit = fit(&Widths { chip: None, ..widths(1280.0, 100.0) });
        assert!(!fit.chip);
    }

    #[test]
    fn a_tiny_window_gives_the_title_nothing() {
        assert_eq!(fit(&widths(300.0, 100.0)).title_max, 0.0);
    }
}
