//! Import Playlist… (S3.6.2): playlist files the user picks become saved playlists, each under
//! its file's name (numbered when taken), with a toast that says how it went.

use std::path::PathBuf;

use gpui::{AppContext as _, AsyncWindowContext, Context, PathPromptOptions, WeakEntity, Window};

use super::super::MainWindow;
use crate::opening::Cancel;
use crate::playlists::{self, Draft, import};
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Asks for playlist files and imports each of them.
    pub(super) fn import_playlists(window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Import".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            for path in paths {
                import_one(&this, path, cx).await;
            }
        })
        .detach();
    }
}

/// Reads the playlist at `path` in the background, saves it, and says so on `main`.
async fn import_one(main: &WeakEntity<MainWindow>, path: PathBuf, cx: &mut AsyncWindowContext) {
    let read = path.clone();
    let imported =
        cx.background_spawn(async move { import::read(&read, &Cancel::default()) }).await;
    let toast = match imported {
        Err(error) => Toast::new(Level::Error, error.message(&path)),
        Ok(imported) => {
            let draft = Draft {
                title: imported.title.clone(),
                thumbnail: None,
                items: imported.items.clone(),
            };
            let Ok(saved) = cx.update(|_, cx| playlists::create(&draft, cx)) else { return };
            match saved.await {
                Some(title) => Toast::new(Level::Success, imported.message(&title)),
                None => Toast::new(Level::Error, "Couldn't save the imported playlist."),
            }
        }
    };
    main.update(cx, |main, cx| main.show_toast(toast, cx)).ok();
}
