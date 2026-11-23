//! Moving around inside media rather than along it (S8.9): the programs (services) of an
//! MPEG-TS stream or DVB broadcast, the titles of a disc, and a DVD or Blu-ray menu's buttons.

use std::time::Duration;

/// One program (service) of a transport stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// Its id (the service id), what selects it.
    pub id: i64,
    /// Its name, as the stream's service description gives it.
    pub name: Option<String>,
}

impl Program {
    /// Its name, or "Program" and its id.
    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("Program {}", self.id))
    }
}

/// The open media's programs, and which one plays.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Programs {
    /// Every program, in the stream's order.
    pub all: Vec<Program>,
    /// The id of the program playing.
    pub current: Option<i64>,
}

impl Programs {
    /// The program after the one playing, wrapping to the first; `None` with fewer than two.
    pub fn next(&self) -> Option<&Program> {
        if self.all.len() < 2 {
            return None;
        }
        let at = self.all.iter().position(|program| Some(program.id) == self.current);
        Some(&self.all[at.map_or(0, |at| (at + 1) % self.all.len())])
    }

    /// The program playing.
    pub fn playing(&self) -> Option<&Program> {
        self.all.iter().find(|program| Some(program.id) == self.current)
    }
}

/// One title of a disc: a film, an episode, an extra, or a menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Title {
    /// Its name, if the disc gives one.
    pub name: Option<String>,
    /// How long it is; `None` when the disc doesn't say.
    pub duration: Option<Duration>,
    /// Whether it is one of the disc's menus.
    pub is_menu: bool,
}

/// The open media's titles, and which one plays. Files have none or one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Titles {
    /// Every title, in the disc's order.
    pub all: Vec<Title>,
    /// The index of the title playing.
    pub current: Option<usize>,
}

impl Titles {
    /// Whether a menu is showing, so the arrow keys and Enter belong to it.
    pub fn in_menu(&self) -> bool {
        self.current.and_then(|at| self.all.get(at)).is_some_and(|title| title.is_menu)
    }

    /// The disc's first menu, which the Disc Menu command goes to.
    pub fn menu(&self) -> Option<usize> {
        self.all.iter().position(|title| title.is_menu)
    }

    /// What lists call the title at `index`: its name, "Menu", or "Title" and its number,
    /// counted without the menus.
    pub fn label(&self, index: usize) -> String {
        let Some(title) = self.all.get(index) else { return String::new() };
        if let Some(name) = title.name.as_ref().filter(|name| !name.trim().is_empty()) {
            return name.clone();
        }
        if title.is_menu {
            return "Menu".to_owned();
        }
        let number = self.all[..index].iter().filter(|title| !title.is_menu).count() + 1;
        format!("Title {number}")
    }
}

/// A key pressed in a disc's menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    /// Press the highlighted button.
    Activate,
    /// Move the highlight up.
    Up,
    /// Move the highlight down.
    Down,
    /// Move the highlight left.
    Left,
    /// Move the highlight right.
    Right,
}

/// Where to move inside the open media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    /// Play the program with this id.
    Program(i64),
    /// Play the title at this index.
    Title(usize),
    /// Press a key in the menu showing.
    Menu(MenuKey),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(id: i64) -> Program {
        Program { id, name: None }
    }

    #[test]
    fn the_next_program_wraps_and_needs_two() {
        let mut programs =
            Programs { all: vec![program(1), program(2), program(5)], current: Some(5) };
        assert_eq!(programs.next().map(|p| p.id), Some(1));
        programs.current = Some(1);
        assert_eq!(programs.next().map(|p| p.id), Some(2));
        programs.current = None;
        assert_eq!(programs.next().map(|p| p.id), Some(1));
        programs.all.truncate(1);
        assert_eq!(programs.next(), None);
        assert_eq!(program(7).label(), "Program 7");
    }

    #[test]
    fn titles_are_labelled_and_menus_found() {
        let title = |name: Option<&str>, is_menu| Title {
            name: name.map(str::to_owned),
            duration: None,
            is_menu,
        };
        let titles = Titles {
            all: vec![title(None, true), title(None, false), title(Some("Extras"), false)],
            current: Some(0),
        };
        assert!(titles.in_menu());
        assert_eq!(titles.menu(), Some(0));
        assert_eq!(titles.label(0), "Menu");
        assert_eq!(titles.label(1), "Title 1");
        assert_eq!(titles.label(2), "Extras");
        assert!(!Titles { current: Some(1), ..titles }.in_menu());
    }
}
