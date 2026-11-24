//! A folder row's menu (S4.8.2; `library/menus/folder_context_menu.png`): Add File and Add
//! Folder, which copy what is picked into the folder ([`copy_in`]), Edit Subtitle Tracks (the
//! Add Subtitles dialog To This Folder, S6.4), Resync (S4.2.5) and Info.
//!
//! The design's Edit Audio Tracks waits for a design of which audio file goes with which video
//! (Add Audio Tracks on a file adds one); the menu leaves it out until then.

use std::path::{Path, PathBuf};

use gpui::{AppContext as _, Context, PathPromptOptions, SharedString, WeakEntity, Window};

use super::MainWindow;
use super::item_menu::{on_all, on_one};
use crate::app::media::Media;
use crate::assets::icons;
use crate::library::{self, copy_in, folder_view, properties};
use crate::main_window::add_subtitles::Target;
use crate::settings;
use crate::ui::menu::MenuItem;
use crate::ui::toast::{Level, Toast};

/// What Add File and Add Folder ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Pick {
    Files,
    Folder,
}

/// The folder menu's items for `folders`.
pub(super) fn items(main: &WeakEntity<MainWindow>, folders: &[PathBuf]) -> Vec<MenuItem> {
    vec![
        on_one(main, folders, tr!("Add File"), icons::ADD, |_, folder, window, cx| {
            MainWindow::copy_into_folder(folder.clone(), Pick::Files, window, cx);
        }),
        on_one(main, folders, tr!("Add Folder"), icons::NEW_FOLDER, |_, folder, window, cx| {
            MainWindow::copy_into_folder(folder.clone(), Pick::Folder, window, cx);
        }),
        on_one(
            main,
            folders,
            tr!("Edit Subtitle Tracks"),
            icons::SUBTITLES,
            |main, folder, window, cx| {
                main.show_add_subtitles(Target::Folder(folder.clone()), window, cx);
            },
        ),
        on_all(main, folders, tr!("Resync"), icons::REFRESH, |main, folders, _, cx| {
            for folder in folders {
                library::resync(folder, cx);
            }
            main.reload_folder(cx);
        }),
        on_one(main, folders, tr!("Info"), icons::INFO, |_, folder, window, cx| {
            MainWindow::show_folder_info(folder.clone(), window, cx);
        }),
    ]
}

impl MainWindow {
    /// Asks for files or a folder, as `pick` says, and copies them into `folder`.
    fn copy_into_folder(folder: PathBuf, pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: pick == Pick::Files,
            directories: pick == Pick::Folder,
            multiple: pick == Pick::Files,
            prompt: Some(tr!("Add").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(sources))) = paths.await else { return };
            let to = folder.clone();
            let copy = cx.background_spawn(async move { copy_in::copy_into(&sources, &to) });
            let copied = copy.await;
            this.update(cx, |this, cx| {
                this.show_toast(copied_toast(&copied, &folder), cx);
                this.reload_folder(cx);
            })
            .ok();
        })
        .detach();
    }

    /// Reads what `folder` holds in the background, and shows it.
    fn show_folder_info(folder: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let show_hidden = settings::get(cx).prefs.library.show_hidden_files;
        let read = cx.background_spawn(async move { folder_rows(&folder, show_hidden) });
        cx.spawn_in(window, async move |this, cx| {
            let rows = read.await;
            this.update_in(cx, |this, window, cx| {
                this.show_info_dialog(tr!("Info"), rows, window, cx);
            })
            .ok();
        })
        .detach();
    }
}

/// What Info lists for `folder`: its name and place, what it holds, and its media's size.
/// Blocks on the disk.
fn folder_rows(folder: &Path, show_hidden: bool) -> Vec<(&'static str, SharedString)> {
    let name = Media::File(folder.to_path_buf()).display_name();
    let place = folder.parent().unwrap_or(folder).display().to_string();
    let mut rows = vec![(tr!("Name"), name.into()), (tr!("Location"), place.into())];
    match folder_view::read_disk(folder, show_hidden) {
        Ok(on_disk) => {
            let bytes = on_disk.files.iter().filter_map(|f| f.metadata().ok()).map(|m| m.len());
            rows.push((tr!("Subfolders"), crate::i18n::number(on_disk.folders.len()).into()));
            rows.push((tr!("Media files"), crate::i18n::number(on_disk.files.len()).into()));
            rows.push((tr!("Size"), properties::size(bytes.sum()).into()));
        }
        Err(_) => rows.push((tr!("Status"), tr!("This folder is no longer available.").into())),
    }
    rows
}

/// What the toast says after copying into `folder`: how many files, or what failed.
fn copied_toast(copied: &copy_in::Copied, folder: &Path) -> Toast {
    let into = Media::File(folder.to_path_buf()).display_name();
    if let Some((source, error)) = copied.failed.first() {
        let name = Media::File(source.clone()).display_name();
        return Toast::new(
            Level::Error,
            tr!("Couldn't copy “{name}” into “{into}”: {error}", name, into, error),
        );
    }
    Toast::new(
        Level::Success,
        trn!(
            "Copied {count} file into “{into}”.",
            "Copied {count} files into “{into}”.",
            copied.files,
            into
        ),
    )
}
