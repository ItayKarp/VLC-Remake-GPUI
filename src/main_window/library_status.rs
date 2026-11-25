//! The library's media folders in the empty state (Settings → Library lists them too, S5.9.3):
//! each folder's status and scan progress
//! (S4.2.1, S4.2.6), with Resync (S4.2.5; the folder menu of S4.8.2 has it too), and a way out when
//! the onboarding folder doesn't exist.

use std::path::{Path, PathBuf};

use gpui::{
    App, ClickEvent, Context, Div, PathPromptOptions, SharedString, Window, div, prelude::*, px,
};

use super::MainWindow;
use crate::db::roots::{RootKind, RootStatus};
use crate::i18n::DirectionalStyle;
use crate::library::indexer::RootState;
use crate::library::root_text::status_text;
use crate::library::{self};
use crate::logging;
use crate::settings;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::Button;
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Whether the media folders' rows may be on screen: in the empty state, or on the library's
    /// Home when it has nothing yet. Otherwise the indexer's reports change nothing shown.
    pub(super) fn shows_library_folders(&self, cx: &App) -> bool {
        self.showing_video(cx).is_none() || self.library_panel.is_open()
    }
}

/// One row per media folder; nothing before the library has any.
pub(super) fn render(p: &Palette, cx: &mut Context<MainWindow>) -> Option<Div> {
    let roots = library::roots(cx).to_vec();
    if roots.is_empty() || !library::is_enabled(cx) {
        return None;
    }
    let rows: Vec<Div> =
        roots.iter().enumerate().map(|(index, root)| render_root(index, root, p, cx)).collect();
    Some(div().w(px(520.)).max_w_full().flex_row_start().flex_col().gap(px(8.)).children(rows))
}

fn render_root(index: usize, root: &RootState, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let name = SharedString::from(settings::display_path(&root.path));
    let text = div()
        .flex_1()
        .min_w_0()
        .flex_row_start()
        .flex_col()
        .child(div().text_size(text_px(13.)).text_color(p.text_strong).truncate().child(name))
        .child(div().text_size(text_px(12.)).text_color(p.text_muted).child(status_text(root)));
    let mut row = div().flex_row_start().items_center().gap(px(8.)).child(text);
    let path = root.path.clone();
    if root.status == RootStatus::Missing && root.kind == RootKind::Video {
        row = row.children(missing_folder_buttons(index, &path, cx));
    } else if root.indexing.is_none() && root.status != RootStatus::Missing {
        let resync = Button::new(("resync", index), tr!("Resync"))
            .on_click(move |_: &ClickEvent, _, cx: &mut App| library::resync(&path, cx));
        row = row.child(resync);
    }
    row
}

/// "Choose another folder" and "Create this folder", for a video folder that isn't there.
fn missing_folder_buttons(index: usize, path: &Path, cx: &mut Context<MainWindow>) -> [Button; 2] {
    let (old, missing) = (path.to_path_buf(), path.to_path_buf());
    let main = cx.entity().downgrade();
    let choose = Button::new(("choose-folder", index), tr!("Choose another folder")).on_click(
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            let old = old.clone();
            main.update(cx, |_, cx| choose_folder(old, window, cx)).ok();
        },
    );
    let this = cx.entity().downgrade();
    let create = Button::new(("create-folder", index), tr!("Create this folder")).on_click(
        move |_: &ClickEvent, _, cx: &mut App| {
            this.update(cx, |_, cx| create_folder(missing.clone(), cx)).ok();
        },
    );
    [choose, create]
}

/// Asks for a folder to index in place of `old`.
fn choose_folder(old: PathBuf, window: &mut Window, cx: &mut Context<MainWindow>) {
    let paths = cx.prompt_for_paths(PathPromptOptions {
        files: false,
        directories: true,
        multiple: false,
        prompt: Some(tr!("Choose").into()),
    });
    cx.spawn_in(window, async move |_, cx| {
        let Ok(Ok(Some(paths))) = paths.await else { return };
        let Some(new) = paths.into_iter().next() else { return };
        cx.update(|_, cx| library::replace_media_folder(&old, new, cx)).ok();
    })
    .detach();
}

/// Creates the missing folder at `path`, then indexes it.
fn create_folder(path: PathBuf, cx: &mut Context<MainWindow>) {
    let target = path.clone();
    let created = cx.background_spawn(async move { std::fs::create_dir_all(&target) });
    cx.spawn(async move |this, cx| {
        let result = created.await;
        this.update(cx, |main, cx| match result {
            Ok(()) => library::resync(&path, cx),
            Err(error) => {
                tracing::warn!("library: couldn't create {}: {error}", logging::path(&path));
                let folder = settings::display_path(&path);
                let message = tr!("Couldn't create “{folder}”.", folder);

                main.show_toast(Toast::new(Level::Error, message), cx);
            }
        })
        .ok();
    })
    .detach();
}
