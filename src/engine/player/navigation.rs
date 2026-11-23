//! The [`Player`]'s programs, titles and disc menus (S8.9): what the backend reports, and moving
//! between them.

use gpui::Context;

use super::{Player, PlayerEvent};
use crate::engine::navigation::{MenuKey, Navigation, Program, Programs, Titles};

impl Player {
    /// The open media's programs; empty unless it is a transport stream with several.
    pub fn programs(&self) -> &Programs {
        &self.info.programs
    }

    /// The open media's titles; files have none or one, discs several.
    pub fn titles(&self) -> &Titles {
        &self.info.titles
    }

    /// Plays the program with `id`.
    pub fn select_program(&mut self, id: i64) {
        if self.media.is_some() {
            self.with_backend(|backend| backend.navigate(Navigation::Program(id)));
        }
    }

    /// Plays the program after the one playing (X), and returns it; `None` when there is no
    /// other.
    pub fn next_program(&mut self) -> Option<Program> {
        let next = self.info.programs.next()?.clone();
        self.select_program(next.id);
        Some(next)
    }

    /// Plays the title at `index`.
    pub fn select_title(&mut self, index: usize) {
        if index < self.info.titles.all.len() {
            self.with_backend(|backend| backend.navigate(Navigation::Title(index)));
        }
    }

    /// Goes to the disc's menu; `false` when it has none.
    pub fn disc_menu(&mut self) -> bool {
        let Some(menu) = self.info.titles.menu() else { return false };
        self.select_title(menu);
        true
    }

    /// Presses `key` in the disc menu showing; `false`, doing nothing, when none shows.
    pub fn press_menu_key(&mut self, key: MenuKey) -> bool {
        if !self.info.titles.in_menu() {
            return false;
        }
        self.with_backend(|backend| backend.navigate(Navigation::Menu(key)));
        true
    }

    /// Keeps the programs the backend reported, and says when they changed.
    pub(super) fn programs_reported(&mut self, programs: Programs, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.info.programs, programs) != self.info.programs {
            cx.emit(PlayerEvent::ProgramsChanged);
        }
    }

    /// Keeps the titles the backend reported, and says when they changed.
    pub(super) fn titles_reported(&mut self, titles: Titles, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.info.titles, titles) != self.info.titles {
            cx.emit(PlayerEvent::TitlesChanged);
        }
    }
}
