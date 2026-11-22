//! The icon rail (S4.5.4, S4.5.7): its items and how each icon is drawn for the current route.
//!
//! Neither "+" nor History is designed. Here "+" adds media: it opens a menu to add a folder to
//! the library or open files. History is a section like the others, listing what was played
//! recently.

use super::route::{Route, Section};

/// One item on the rail, top to bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailItem {
    /// A section, which the item opens at its root.
    Section(Section),
    /// "+": add media.
    Add,
}

impl RailItem {
    /// Every item, in the rail's order.
    pub const ALL: [Self; 7] = [
        Self::Section(Section::Home),
        Self::Section(Section::Videos),
        Self::Section(Section::Music),
        Self::Section(Section::Network),
        Self::Section(Section::Playlists),
        Self::Section(Section::History),
        Self::Add,
    ];
}

/// How a rail icon is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailState {
    /// An outline: the library is in another section.
    Outline,
    /// Filled orange: the library is at the section's root.
    Root,
    /// Filled grey: the library is deeper in the section.
    Inside,
}

/// How `item` is drawn while the library is at `route`. "+" is always an outline.
pub fn state(item: RailItem, route: &Route) -> RailState {
    match item {
        RailItem::Section(section) if section == route.section() => {
            if route.is_section_root() {
                RailState::Root
            } else {
                RailState::Inside
            }
        }
        RailItem::Section(_) | RailItem::Add => RailState::Outline,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::route::MusicCategory;

    #[test]
    fn the_current_section_is_filled() {
        let music = RailItem::Section(Section::Music);
        assert_eq!(state(music, &Section::Music.root()), RailState::Root);
        assert_eq!(state(music, &MusicCategory::Songs.route()), RailState::Inside);
        assert_eq!(state(music, &Route::Home), RailState::Outline);
    }

    #[test]
    fn add_is_never_filled() {
        for section in Section::ALL {
            assert_eq!(state(RailItem::Add, &section.root()), RailState::Outline);
        }
    }

    #[test]
    fn only_one_item_is_filled() {
        let route = Route::Videos(Some("/media/Shows".into()));
        let filled =
            RailItem::ALL.iter().filter(|item| state(**item, &route) != RailState::Outline);
        assert_eq!(filled.count(), 1);
    }
}
