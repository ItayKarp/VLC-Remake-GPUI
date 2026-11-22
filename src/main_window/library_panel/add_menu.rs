//! The rail's "+" menu (S4.5.7, not designed): add a folder to the library, or open files, a
//! folder or a network stream (S4.11) to play. The listing's "+ Add" rows (S4.6.1) add a folder
//! the same way, to the video or the music folders.

use std::path::PathBuf;

use gpui::{AnyElement, App, Context, DismissEvent, PathPromptOptions, WeakEntity, Window};

use super::{MainWindow, OpenMenu};
use crate::assets::icons;
use crate::keymap::actions::{OpenFile, OpenFolder, OpenNetworkStream};
use crate::main_window::main_menu::dispatching;
use crate::navigation::route::{MusicRoute, Route};
use crate::settings;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};

impl MainWindow {
    /// Opens or closes the "+" menu beside its button.
    pub(super) fn toggle_add_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.library_panel.add_menu.take().is_some() {
            cx.notify();
            return;
        }
        let items = items(&cx.entity().downgrade(), cx);
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.library_panel.add_menu = None;
            cx.notify();
        });
        self.library_panel.add_menu = Some((menu, subscription));
        cx.notify();
    }
}

fn items(main: &WeakEntity<MainWindow>, cx: &App) -> Vec<MenuItem> {
    let main = main.clone();
    vec![
        MenuItem::action("Add Folder to Library…", move |_, cx| {
            add_folder(main.clone(), Shelf::Videos, cx);
        })
        .icon(icons::FOLDER),
        MenuItem::Separator,
        dispatching(&OpenFile, "Open File(s)…", cx).icon(icons::FILE),
        dispatching(&OpenFolder, "Open Folder…", cx).icon(icons::FOLDER),
        dispatching(&OpenNetworkStream, "Open Network Stream…", cx).icon(icons::STREAMING),
    ]
}

/// Which of the library's folder lists a folder joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shelf {
    /// The video folders (`local.media_folders`).
    Videos,
    /// The music folders (`local.music_folders`).
    Music,
}

/// Asks for a folder, adds it to the library's `shelf` folders, and shows it.
pub(super) fn add_folder(main: WeakEntity<MainWindow>, shelf: Shelf, cx: &mut App) {
    let options = PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some("Add to Library".into()),
    };
    let picked = cx.prompt_for_paths(options);
    cx.spawn(async move |cx| {
        let Ok(Ok(Some(mut paths))) = picked.await else { return };
        let Some(folder) = paths.pop() else { return };
        cx.update(|cx| {
            add_library_folder(&folder, shelf, cx);
            let route = match shelf {
                Shelf::Videos => Route::Videos(Some(folder)),
                Shelf::Music => Route::Music(MusicRoute::Songs(Some(folder))),
            };
            main.update(cx, |main, cx| main.navigate(route, cx)).ok();
        });
    })
    .detach();
}

/// Adds `folder` to the `shelf` folders the library indexes, unless it is there already.
fn add_library_folder(folder: &PathBuf, shelf: Shelf, cx: &mut App) {
    settings::update(cx, |s| {
        let folders = match shelf {
            Shelf::Videos => &mut s.local.media_folders,
            Shelf::Music => &mut s.local.music_folders,
        };
        if !folders.contains(folder) {
            folders.push(folder.clone());
        }
    });
}

/// The "+" menu beside its button, while open.
pub(super) fn render(menu: &OpenMenu, button: &BoundsTracker) -> Option<AnyElement> {
    let (menu, _) = menu.as_ref()?;
    Some(popover::attached(button.bounds()?, Side::Right, 4., 1, menu.clone()))
}
