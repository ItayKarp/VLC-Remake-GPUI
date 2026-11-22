//! The library's media folders in the empty state, until the library views exist (S4.5, S4.6)
//! and Settings → Library lists them (S5.9.3): each folder's status and scan progress
//! (S4.2.1, S4.2.6), with Resync (S4.2.5; the folder menu of S4.8.2 has it too), and a way out when
//! the onboarding folder doesn't exist.

use std::path::{Path, PathBuf};

use gpui::{
    App, ClickEvent, Context, Div, PathPromptOptions, SharedString, Window, div, prelude::*, px,
};

use super::MainWindow;
use crate::db::roots::{RootKind, RootStatus};
use crate::library::indexer::RootState;
use crate::library::progress::group_thousands;
use crate::library::{self};
use crate::logging;
use crate::settings;
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::toast::{Level, Toast};

/// One row per media folder; nothing before the library has any.
pub(super) fn render(p: &Palette, cx: &mut Context<MainWindow>) -> Option<Div> {
    let roots = library::roots(cx).to_vec();
    if roots.is_empty() {
        return None;
    }
    let rows: Vec<Div> =
        roots.iter().enumerate().map(|(index, root)| render_root(index, root, p, cx)).collect();
    Some(div().w(px(520.)).max_w_full().flex().flex_col().gap(px(8.)).children(rows))
}

fn render_root(index: usize, root: &RootState, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let name = SharedString::from(settings::display_path(&root.path));
    let text = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().text_size(px(13.)).text_color(p.text_strong).truncate().child(name))
        .child(div().text_size(px(12.)).text_color(p.text_muted).child(status_text(root)));
    let mut row = div().flex().items_center().gap(px(8.)).child(text);
    let path = root.path.clone();
    if root.status == RootStatus::Missing && root.kind == RootKind::Video {
        row = row.children(missing_folder_buttons(index, &path, cx));
    } else if root.indexing.is_none() && root.status != RootStatus::Missing {
        let resync = Button::new(("resync", index), "Resync")
            .on_click(move |_: &ClickEvent, _, cx: &mut App| library::resync(&path, cx));
        row = row.child(resync);
    }
    row
}

/// What the row says about `root`.
pub fn status_text(root: &RootState) -> SharedString {
    if let Some(progress) = root.indexing {
        return progress.label().into();
    }
    let items = match root.files {
        1 => "1 item".to_owned(),
        n => format!("{} items", group_thousands(n)),
    };
    let text = match root.status {
        RootStatus::Ok => items,
        RootStatus::Slow => format!("{items} · Slow network folder"),
        RootStatus::Missing if root.files > 0 => {
            format!("{items} · Drive not connected or folder moved")
        }
        RootStatus::Missing => "This folder doesn't exist".to_owned(),
        RootStatus::PermissionDenied => "VLC isn't allowed to read this folder".to_owned(),
    };
    match root.unreadable {
        0 => text.into(),
        1 => format!("{text} · 1 folder couldn't be read").into(),
        n => format!("{text} · {n} folders couldn't be read").into(),
    }
}

/// "Choose another folder" and "Create this folder", for a video folder that isn't there.
fn missing_folder_buttons(index: usize, path: &Path, cx: &mut Context<MainWindow>) -> [Button; 2] {
    let (old, missing) = (path.to_path_buf(), path.to_path_buf());
    let main = cx.entity().downgrade();
    let choose = Button::new(("choose-folder", index), "Choose another folder").on_click(
        move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            let old = old.clone();
            main.update(cx, |_, cx| choose_folder(old, window, cx)).ok();
        },
    );
    let this = cx.entity().downgrade();
    let create = Button::new(("create-folder", index), "Create this folder").on_click(
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
        prompt: Some("Choose".into()),
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
                let message = format!("Couldn't create “{}”.", settings::display_path(&path));
                main.show_toast(Toast::new(Level::Error, message), cx);
            }
        })
        .ok();
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::progress::Progress;

    fn root(status: RootStatus, files: usize) -> RootState {
        RootState {
            path: "/v".into(),
            kind: RootKind::Video,
            status,
            files,
            unreadable: 0,
            indexing: None,
        }
    }

    #[test]
    fn rows_say_what_each_folder_is_doing() {
        assert_eq!(status_text(&root(RootStatus::Ok, 1)), "1 item");
        assert_eq!(status_text(&root(RootStatus::Ok, 12_345)), "12,345 items");
        assert_eq!(status_text(&root(RootStatus::Missing, 0)), "This folder doesn't exist");
        let unplugged = root(RootStatus::Missing, 3);
        assert_eq!(status_text(&unplugged), "3 items · Drive not connected or folder moved");
        let indexing = RootState {
            indexing: Some(Progress { found: 1_234, estimate: 5_000 }),
            ..root(RootStatus::Ok, 0)
        };
        assert_eq!(status_text(&indexing), "Indexing 1,234 of ~5,000");
        let partly = RootState { unreadable: 2, ..root(RootStatus::Slow, 2) };
        assert_eq!(
            status_text(&partly),
            "2 items · Slow network folder · 2 folders couldn't be read"
        );
    }
}
