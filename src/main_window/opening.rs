//! Opening media in the main window (S1.3): the Open File(s) and Open Folder dialogs, drops,
//! paste and launches all end in [`MainWindow::open`], which resolves the items off the UI
//! thread ([`opening::resolve`]) and then plays them.
//!
//! While items resolve (a large folder, a sleeping network drive), the window shows
//! "Opening…" with a Cancel button. Opening something new in place of the list cancels what
//! is still resolving; adding to the list (`--enqueue`) doesn't.

use gpui::{
    ClipboardEntry, Context, ExternalPaths, PathPromptOptions, SharedString, Task, Window,
    prelude::*,
};

use super::MainWindow;
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::opening::{self, Cancel, Resolved, paste, recent};
use crate::ui::toast::{Level, Toast};

/// Items being resolved before they play.
pub(super) struct Job {
    id: u64,
    placement: Placement,
    cancel: Cancel,
    /// What is being opened, for "Opening …".
    pub(super) label: SharedString,
    _task: Task<()>,
}

/// Which dialog to show.
#[derive(Clone, Copy)]
enum Pick {
    Files,
    Folder,
}

impl MainWindow {
    /// Opens what was asked for (S0.6.2, S1.3): switches to fullscreen if asked, then resolves
    /// the items in the background and plays them, replacing the list or adding to it.
    pub fn open(&mut self, request: OpenRequest, window: &mut Window, cx: &mut Context<Self>) {
        if request.fullscreen && !window.is_fullscreen() {
            window.toggle_fullscreen();
        }
        if request.items.is_empty() {
            return;
        }
        if request.placement == Placement::Replace {
            self.cancel_opening(cx);
        }
        let cancel = Cancel::default();
        self.next_job += 1;
        let id = self.next_job;
        let label = job_label(&request.items);
        let placement = request.placement;
        let job_cancel = cancel.clone();
        let task = cx.spawn_in(window, async move |this, cx| {
            let items = request.items;
            let resolved =
                cx.background_spawn(async move { opening::resolve::resolve(items, &job_cancel) });
            let resolved = resolved.await;
            this.update_in(cx, |this, window, cx| this.finish_job(id, resolved, window, cx)).ok();
        });
        self.jobs.push(Job { id, placement, cancel, label, _task: task });
        cx.notify();
    }

    /// Stops resolving every pending open.
    pub(super) fn cancel_opening(&mut self, cx: &mut Context<Self>) {
        for job in self.jobs.drain(..) {
            job.cancel.cancel();
        }
        cx.notify();
    }

    /// Plays what job `id` resolved to, unless it was cancelled meanwhile.
    fn finish_job(
        &mut self,
        id: u64,
        resolved: Option<Resolved>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.jobs.iter().position(|job| job.id == id) else {
            return;
        };
        let job = self.jobs.remove(index);
        cx.notify();
        let (Some(resolved), placement) = (resolved, job.placement) else {
            return;
        };
        self.report(&resolved, cx);
        recent::record(&resolved.opened, cx);
        if !resolved.media.is_empty() {
            self.play_list(resolved.media, placement, window, cx);
        }
        self.attach_subtitles(&resolved.subtitles, cx);
    }

    /// Tells the user what was left out and what couldn't be read.
    fn report(&mut self, resolved: &Resolved, cx: &mut Context<Self>) {
        for (item, problem) in &resolved.problems {
            self.show_toast(Toast::new(Level::Error, problem.message(item)), cx);
        }
        if resolved.skipped > 0 {
            let message = match resolved.skipped {
                1 => "1 file skipped: it isn't media.".to_owned(),
                n => format!("{n} files skipped: they aren't media."),
            };
            self.show_toast(Toast::new(Level::Info, message), cx);
        }
    }

    /// Asks for files or a folder to open, and opens what the user picks.
    fn prompt(pick: Pick, window: &mut Window, cx: &mut Context<Self>) {
        let folder = matches!(pick, Pick::Folder);
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: !folder,
            directories: folder,
            multiple: true,
            prompt: Some("Open".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                let items = paths.into_iter().map(Media::from).collect();
                this.open(OpenRequest::replace(items), window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Open File(s) (S1.3.1).
    pub(super) fn prompt_for_files(window: &mut Window, cx: &mut Context<Self>) {
        Self::prompt(Pick::Files, window, cx);
    }

    /// Open Folder (S1.3.1).
    pub(super) fn prompt_for_folder(window: &mut Window, cx: &mut Context<Self>) {
        Self::prompt(Pick::Folder, window, cx);
    }

    /// Files dropped anywhere on the window (S1.3.2).
    pub(super) fn on_drop(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = paths.paths().iter().cloned().map(Media::from).collect();
        self.open(OpenRequest::replace(items), window, cx);
    }

    /// Opens the paths or URLs on the clipboard (S1.3.3): files copied in a file manager, or
    /// text with one path or URL per line.
    pub(super) fn paste(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = cx.read_from_clipboard().map(|item| clipboard_items(item.entries()));
        match items {
            Some(items) if !items.is_empty() => {
                self.open(OpenRequest::replace(items), window, cx);
            }
            _ => self.show_toast(
                Toast::new(Level::Info, "Copy a file path or a URL first, then paste it here."),
                cx,
            ),
        }
    }
}

/// The items on the clipboard: copied files, and paths or URLs in copied text.
fn clipboard_items(entries: &[ClipboardEntry]) -> Vec<Media> {
    let home = dirs::home_dir();
    entries
        .iter()
        .flat_map(|entry| match entry {
            ClipboardEntry::ExternalPaths(paths) => {
                paths.paths().iter().cloned().map(Media::from).collect()
            }
            ClipboardEntry::String(text) => paste::items(text.text(), home.as_deref()),
            ClipboardEntry::Image(_) => Vec::new(),
        })
        .collect()
}

/// "Opening …" names the item, or counts several.
fn job_label(items: &[Media]) -> SharedString {
    match items {
        [item] => item.display_name().into(),
        items => format!("{} items", items.len()).into(),
    }
}
