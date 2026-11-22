//! How the library shows a section's items (S4.6.4, S4.7.4): a list of rows or a grid of tiles,
//! remembered per section across launches. Lists are the default.

use super::route::Section;

/// How a section's items are shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    /// One item per row (`sidebar-*`).
    #[default]
    List,
    /// Tiles, the name under each (`player_03`).
    Grid,
}

impl View {
    /// How `section` is shown, given the sections remembered as grids.
    pub fn of(section: Section, grid: &[String]) -> Self {
        if grid.iter().any(|name| name == section.label()) { Self::Grid } else { Self::List }
    }

    /// Remembers this as how `section` is shown.
    pub fn remember(self, section: Section, grid: &mut Vec<String>) {
        grid.retain(|name| name != section.label());
        if self == Self::Grid {
            grid.push(section.label().to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_section_remembers_its_own_view() {
        let mut grid = Vec::new();
        assert_eq!(View::of(Section::Videos, &grid), View::List);
        View::Grid.remember(Section::Videos, &mut grid);
        View::Grid.remember(Section::Videos, &mut grid);
        assert_eq!(grid, ["Videos"]);
        assert_eq!(View::of(Section::Videos, &grid), View::Grid);
        assert_eq!(View::of(Section::Music, &grid), View::List);
        View::List.remember(Section::Videos, &mut grid);
        assert_eq!(View::of(Section::Videos, &grid), View::List);
    }
}
