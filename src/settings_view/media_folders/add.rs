//! Adding a media folder from the list: asking for it, placing it by its canonical path off the
//! UI thread, then refusing it, asking first, or adding it.

use std::path::PathBuf;

use gpui::{Context, PathPromptOptions, PromptLevel, Window};

use super::MediaFoldersPanel;
use crate::db::roots::RootKind;
use crate::library::folders::{self, Placement};
use crate::settings::{self, LocalState, display_path};

/// A folder picked to add, and what adding it would do.
struct Checked {
    /// As picked.
    folder: PathBuf,
    placement: Placement,
    /// A whole drive or the home folder.
    broad: bool,
}

impl Checked {
    /// Places `folder` among `local`'s media folders. Reads the disk.
    fn of(folder: PathBuf, local: &LocalState) -> Self {
        let canonical = folders::canonical(&folder);
        let placement = folders::placement(&canonical, &folders::existing(local));
        Self { broad: folders::broad(&canonical), folder, placement }
    }
}

impl MediaFoldersPanel {
    /// Asks for a folder to add to the library's `kind` folders.
    pub(super) fn pick(&mut self, kind: RootKind, window: &mut Window, cx: &mut Context<Self>) {
        self.set_refused(None, cx);
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Add to Library".into()),
        });
        let local = settings::get(cx).local.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else { return };
            let Some(folder) = paths.pop() else { return };
            let checking =
                cx.background_executor().spawn(async move { Checked::of(folder, &local) });
            let checked = checking.await;
            this.update_in(cx, |panel, window, cx| panel.place(kind, checked, window, cx)).ok();
        })
        .detach();
    }

    /// Refuses the folder, asks first, or adds it, as `checked` says.
    fn place(
        &mut self,
        kind: RootKind,
        checked: Checked,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = display_path(&checked.folder);
        match &checked.placement {
            Placement::Duplicate(_) => {
                self.set_refused(Some(format!("“{name}” is in the library already.")), cx);
            }
            Placement::Inside(outer) => {
                let outer = display_path(outer);
                let reason =
                    format!("“{name}” is inside “{outer}”, which the library has already.");
                self.set_refused(Some(reason), cx);
            }
            _ if checked.broad && settings::get(cx).prefs.startup.show_dialogs => {
                confirm_broad(kind, checked, window, cx);
            }
            _ => add(kind, &checked, cx),
        }
    }
}

/// Asks before adding a whole drive or the home folder.
fn confirm_broad(
    kind: RootKind,
    checked: Checked,
    window: &mut Window,
    cx: &mut Context<MediaFoldersPanel>,
) {
    let title = format!("Add all of “{}” to the library?", display_path(&checked.folder));
    let detail = "Indexing a whole drive or your home folder can take a long time. The system \
                  and app folders in it are skipped.";
    let answer =
        window.prompt(PromptLevel::Warning, &title, Some(detail), &["Add Folder", "Cancel"], cx);
    cx.spawn(async move |_, cx| {
        if answer.await == Ok(0) {
            cx.update(|cx| add(kind, &checked, cx));
        }
    })
    .detach();
}

/// Adds the folder to the settings, which the library follows.
fn add(kind: RootKind, checked: &Checked, cx: &mut gpui::App) {
    let folder = &checked.folder;
    settings::update(cx, |s| {
        folders::add(&mut s.local, folder, kind, &checked.placement);
    });
}
