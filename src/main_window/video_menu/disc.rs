//! The video menu's Title ▸ and Program ▸ (S8.9), in Playback ▸ after Chapter ▸: a disc's
//! titles, with Disc Menu, and a transport stream's programs, with Next Program. Each shows only
//! when there is more than one to pick from.

use gpui::{App, WeakEntity};

use super::run;
use crate::engine::navigation::{Programs, Titles};
use crate::keymap::actions as act;
use crate::main_window::MainWindow;
use crate::ui::menu::MenuItem;

/// Title ▸: each title, the playing one checked, then Disc Menu when the disc has one.
pub(super) fn title_menu(
    main: &WeakEntity<MainWindow>,
    titles: &Titles,
    cx: &App,
) -> Option<MenuItem> {
    if titles.all.len() < 2 {
        return None;
    }
    let mut items: Vec<MenuItem> = (0..titles.all.len())
        .map(|index| {
            let main = main.clone();
            MenuItem::radio(titles.label(index), titles.current == Some(index), move |_, cx| {
                main.update(cx, |main, cx| main.select_title(index, cx)).ok();
            })
        })
        .collect();
    if titles.menu().is_some() {
        items.extend([MenuItem::Separator, run(main, &act::DiscMenu, "Disc Menu", cx)]);
    }
    Some(MenuItem::submenu("Title", items))
}

/// Program ▸: each program (service), the playing one checked, then Next Program.
pub(super) fn program_menu(
    main: &WeakEntity<MainWindow>,
    programs: &Programs,
    cx: &App,
) -> Option<MenuItem> {
    if programs.all.len() < 2 {
        return None;
    }
    let mut items: Vec<MenuItem> = programs
        .all
        .iter()
        .map(|program| {
            let (main, id) = (main.clone(), program.id);
            MenuItem::radio(program.label(), programs.current == Some(id), move |_, cx| {
                main.update(cx, |main, cx| main.select_program(id, cx)).ok();
            })
        })
        .collect();
    items.extend([MenuItem::Separator, run(main, &act::NextProgram, "Next Program", cx)]);
    Some(MenuItem::submenu("Program", items))
}
