//! Export… (S3.6.1): a saved playlist written to an `.m3u8` or `.xspf` file.
//!
//! The native save dialog can't hold options, so a small dialog asks for the format and the
//! path style first, then the save dialog asks where. A name typed with the other format's
//! extension gets that format.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, Context, Div, Window, div, prelude::*, px};

use super::super::MainWindow;
use crate::db::playlists::Summary;
use crate::i18n::DirectionalStyle;
use crate::playlists::export::{self, Export, Format, PathStyle};
use crate::playlists::{self};
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::segmented::Segmented;
use crate::ui::toast::{Level, Toast};
use crate::ui::toggle::Checkbox;

/// Space between the dialog's rows.
const ROW_GAP: f32 = 12.;
/// Characters that can't be in a file name on some system; the suggested name avoids them.
const UNSAFE_IN_FILE_NAMES: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// What the export dialog asks for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Choice {
    format: Format,
    paths: PathStyle,
}

impl MainWindow {
    /// Asks how to export playlist `summary`, then where.
    pub(in crate::main_window) fn show_export_playlist(
        &mut self,
        summary: &Summary,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let choice = Rc::new(Cell::new(Choice::default()));
        let main = cx.entity().downgrade();
        let export = {
            let (choice, summary) = (choice.clone(), summary.clone());
            DialogButton::new(tr!("Export…")).primary().on_click(move |window, cx| {
                let choice = choice.get();
                main.update(cx, |_, cx| Self::export_playlist(&summary, choice, window, cx)).ok();
            })
        };
        let body = move |_: &mut Window, _: &mut App| options(&choice).into_any_element();
        let buttons = vec![DialogButton::new(tr!("Cancel")), export];
        let title = tr!("Export “{title}”", title = summary.title);
        let dialog = cx.new(|cx| Dialog::new(title, body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Asks where to save playlist `summary`, writes it there, and says how it went.
    fn export_playlist(
        summary: &Summary,
        choice: Choice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = playlists::export_items(summary.id, cx);
        let folder = dirs::document_dir().or_else(dirs::home_dir).unwrap_or_default();
        let name = format!("{}.{}", file_name(&summary.title), choice.format.extension());
        let destination = cx.prompt_for_new_path(&folder, Some(&name));
        let title = summary.title.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(path))) = destination.await else { return };
            let job = items.await.map(|items| Job { path, title, items, choice });
            let toast = match job {
                Some(job) => written_toast(cx.background_spawn(async move { job.run() }).await),
                None => Toast::new(Level::Error, tr!("Couldn't read the playlist to export it.")),
            };
            this.update(cx, |this, cx| this.show_toast(toast, cx)).ok();
        })
        .detach();
    }
}

/// One export to write, off the UI thread.
struct Job {
    path: PathBuf,
    title: String,
    items: Vec<export::Item>,
    choice: Choice,
}

impl Job {
    /// Writes the playlist, and answers with the path it went to.
    fn run(self) -> std::io::Result<PathBuf> {
        let format = Format::for_path(&self.path, self.choice.format);
        let export = Export { title: &self.title, items: &self.items, paths: self.choice.paths };
        export::write(&self.path, &export::render(&export, format, &self.path)).map(|()| self.path)
    }
}

/// "Exported to “Mix.m3u8”.", or why it couldn't be.
fn written_toast(written: std::io::Result<PathBuf>) -> Toast {
    match written {
        Ok(path) => {
            let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy();
            Toast::new(Level::Success, tr!("Exported to “{name}”.", name))
        }
        Err(error) => {
            tracing::warn!("playlists: export failed: {error}");
            Toast::new(Level::Error, export::error_message(&error))
        }
    }
}

/// The dialog's body: the format, and whether paths are relative.
fn options(choice: &Rc<Cell<Choice>>) -> Div {
    let current = choice.get();
    let selected = Format::ALL.iter().position(|f| *f == current.format).unwrap_or_default();
    let format = {
        let choice = choice.clone();
        Segmented::new("export-format", tr!("Format"), Format::ALL.map(Format::label), selected)
            .on_select(move |index, window, _| {
                let format = Format::ALL.get(*index).copied().unwrap_or_default();
                choice.set(Choice { format, ..choice.get() });
                window.refresh();
            })
    };
    let relative = {
        let choice = choice.clone();
        Checkbox::new(
            "export-relative",
            current.paths == PathStyle::Relative,
            tr!("Relative paths"),
        )
        .on_change(move |checked, window, _| {
            let paths = if *checked { PathStyle::Relative } else { PathStyle::Absolute };
            choice.set(Choice { paths, ..choice.get() });
            window.refresh();
        })
    };
    let hint = match current.paths {
        PathStyle::Relative => {
            tr!("The playlist keeps working when it moves along with its files.")
        }
        PathStyle::Absolute => {
            tr!("The playlist keeps working wherever it moves on this computer.")
        }
    };
    div().flex_row_start().flex_col().gap(px(ROW_GAP)).child(format).child(relative).child(hint)
}

/// `title` as a file name: characters some systems refuse become `-`.
fn file_name(title: &str) -> String {
    title.replace(UNSAFE_IN_FILE_NAMES, "-")
}
