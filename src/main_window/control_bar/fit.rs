//! Fitting the bar to a narrow window (S2.2.5): items give way one at a time, lowest priority
//! first, until the rest fit. Buttons move into an overflow `…` menu; readouts are hidden.
//! Play/Pause always stays, and so do spacers and dividers.

use super::layout::{self, BUTTON, DIVIDER_WIDTH, Dropped};
use crate::player_layout::{Item, Piece};

/// The space at each end of the bar.
pub const PADDING: f32 = 20.;
/// The space between neighbouring items.
pub const ITEM_GAP: f32 = 10.;

/// What the bar shows at a given width.
#[derive(Debug, PartialEq)]
pub struct Fitted {
    /// The pieces to draw, in order, without gaps at the ends or next to each other.
    pub slots: Vec<Piece>,
    /// The items in the overflow menu, in layout order. The `…` button shows when there are any.
    pub overflow: Vec<Item>,
}

/// What of `layout` fits in a bar `available` pixels wide.
pub fn fit(layout: &[Piece], available: f32) -> Fitted {
    let mut dropped = Vec::new();
    for candidate in give_way_order(layout) {
        if width(layout, &dropped) <= available {
            break;
        }
        dropped.push(candidate);
    }
    let overflow = dropped_items(layout, &dropped)
        .filter(|item| layout::fit(*item).dropped == Dropped::Overflow)
        .collect();
    Fitted { slots: kept(layout, &dropped), overflow }
}

/// The indices of the items that may give way, in the order they do.
fn give_way_order(layout: &[Piece]) -> Vec<usize> {
    let mut order: Vec<(usize, Item)> = layout
        .iter()
        .enumerate()
        .filter_map(|(index, slot)| match slot {
            Piece::Item(item) if layout::fit(*item).dropped != Dropped::Never => {
                Some((index, *item))
            }
            _ => None,
        })
        .collect();
    order.sort_by_key(|&(index, item)| (layout::fit(item).priority, std::cmp::Reverse(index)));
    order.into_iter().map(|(index, _)| index).collect()
}

/// How wide the bar is with the items at `dropped` gone.
fn width(layout: &[Piece], dropped: &[usize]) -> f32 {
    let slots = kept(layout, dropped);
    let overflows =
        dropped_items(layout, dropped).any(|item| layout::fit(item).dropped == Dropped::Overflow);
    let mut widths: Vec<f32> = slots.iter().copied().map(slot_width).collect();
    if overflows {
        widths.push(BUTTON);
    }
    let gaps = widths.len().saturating_sub(1) as f32 * ITEM_GAP;
    2. * PADDING + widths.iter().sum::<f32>() + gaps
}

fn slot_width(slot: Piece) -> f32 {
    match slot {
        Piece::Item(item) => layout::fit(item).width,
        Piece::Gap(width) => width,
        Piece::Spacer => 0.,
        Piece::Divider => DIVIDER_WIDTH,
    }
}

fn dropped_items<'a>(layout: &'a [Piece], dropped: &'a [usize]) -> impl Iterator<Item = Item> + 'a {
    layout.iter().enumerate().filter_map(|(index, slot)| match slot {
        Piece::Item(item) if dropped.contains(&index) => Some(*item),
        _ => None,
    })
}

/// The slots left once the items at `dropped` are gone, with the gaps around them tidied: none
/// at either end, and one where two would meet.
fn kept(layout: &[Piece], dropped: &[usize]) -> Vec<Piece> {
    let mut slots: Vec<Piece> = Vec::new();
    for (index, slot) in layout.iter().enumerate() {
        match slot {
            Piece::Item(_) if dropped.contains(&index) => {}
            Piece::Gap(_) if matches!(slots.last(), None | Some(Piece::Gap(_))) => {}
            _ => slots.push(*slot),
        }
    }
    if matches!(slots.last(), Some(Piece::Gap(_))) {
        slots.pop();
    }
    slots
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_layout::Layout;

    fn default_layout() -> Vec<Piece> {
        Layout::designed().rows.remove(0)
    }

    fn items(fitted: &Fitted) -> Vec<Item> {
        fitted
            .slots
            .iter()
            .filter_map(|slot| match slot {
                Piece::Item(item) => Some(*item),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_wide_window_shows_everything() {
        let fitted = fit(&default_layout(), 1920.);
        assert_eq!(fitted.slots, default_layout());
        assert_eq!(fitted.overflow, []);
    }

    #[test]
    fn settings_list_then_subtitles_go_into_the_overflow_menu_first() {
        let full = width(&default_layout(), &[]);
        // The `…` button takes the first item's place, so a second one has to go with it.
        assert_eq!(fit(&default_layout(), full - 1.).overflow, [Item::Settings, Item::List]);
        let fitted = fit(&default_layout(), full - 40.);
        assert_eq!(fitted.overflow, [Item::Subtitles, Item::Settings, Item::List]);
    }

    #[test]
    fn readouts_hide_rather_than_overflow() {
        let fitted = fit(&default_layout(), 400.);
        assert!(!items(&fitted).contains(&Item::Duration));
        assert!(!fitted.overflow.contains(&Item::Duration));
    }

    #[test]
    fn play_pause_always_stays_and_everything_shown_fits() {
        for available in (0..2000).step_by(25) {
            let available = available as f32;
            let fitted = fit(&default_layout(), available);
            assert!(items(&fitted).contains(&Item::PlayPause), "at {available}");
            if items(&fitted).len() > 1 {
                assert!(width_of(&fitted) <= available, "at {available}");
            }
        }
    }

    #[test]
    fn gaps_never_sit_at_the_ends_or_side_by_side() {
        for available in (0..2000).step_by(25) {
            let slots = fit(&default_layout(), available as f32).slots;
            assert!(!matches!(slots.first(), Some(Piece::Gap(_))));
            assert!(!matches!(slots.last(), Some(Piece::Gap(_))));
            let doubled = slots.windows(2).any(|w| matches!(w, [Piece::Gap(_), Piece::Gap(_)]));
            assert!(!doubled, "at {available}");
        }
    }

    /// The width of what `fitted` draws, the `…` button included.
    fn width_of(fitted: &Fitted) -> f32 {
        let mut widths: Vec<f32> = fitted.slots.iter().copied().map(slot_width).collect();
        if !fitted.overflow.is_empty() {
            widths.push(BUTTON);
        }
        2. * PADDING + widths.iter().sum::<f32>() + (widths.len() - 1) as f32 * ITEM_GAP
    }
}
