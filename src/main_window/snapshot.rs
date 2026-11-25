//! Snapshots (S8.4): Shift+S, the macOS View menu and the video menu's Video ▸ save the picture
//! on screen at the size it was decoded at, as Settings → Video's Snapshots rows say (`snapshot`
//! names and writes the file). The picture is read back and saved off the UI thread, then a
//! toast shows its thumbnail with Open, Show in Finder and Copy.
//!
//! Only video takes one: music mode, audio-only media and a visualizer's pictures don't, and
//! nothing happens before the first picture shows. Paused works, as the picture stays on screen.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use gpui::{App, AppContext as _, ClipboardItem, Context, Image, ImageFormat, RenderImage};

use super::MainWindow;
use super::library_panel::show_in_folder_label;
use super::queue_panel::media_title;
use super::video_menu::NOTHING_PLAYING;
use crate::settings::{self, display_path};
use crate::snapshot::{self, Request, Saved};
use crate::ui::toast::{Level, Toast};
use crate::video::Still;

/// Why there is no snapshot of media without a picture.
const NO_VIDEO: &str = tr_noop!("No video to take a snapshot of");
/// Why there is no snapshot before the first picture shows.
const NO_PICTURE_YET: &str = tr_noop!("No picture yet");

/// Why a snapshot wasn't saved.
enum Failure {
    /// The picture couldn't be read back: protected video, or the GPU kept it.
    Unreadable(anyhow::Error),
    /// No folder took the file.
    Unwritable(anyhow::Error),
}

impl MainWindow {
    /// Why Take Snapshot can't run now; `None` when it can.
    pub(super) fn why_no_snapshot(&self, cx: &App) -> Option<&'static str> {
        let text = crate::i18n::text;
        let Some(player) = self.loaded_player(cx) else { return Some(text(NOTHING_PLAYING)) };
        if self.is_music_mode() || player.read(cx).video_size().is_none() {
            return Some(text(NO_VIDEO));
        }
        let shows = self.video.as_ref().is_some_and(|video| video.read(cx).still(cx).is_some());
        (!shows).then(|| text(NO_PICTURE_YET))
    }

    /// Saves the picture on screen, or says over the video why it can't.
    pub(super) fn take_snapshot(&mut self, cx: &mut Context<Self>) {
        if let Some(why) = self.why_no_snapshot(cx) {
            self.show_osd(why, cx);
            return;
        }
        let Some((still, request)) = self.snapshot_request(cx) else { return };
        let saving = cx.background_spawn(async move {
            let picture = still.picture().map_err(Failure::Unreadable)?;
            snapshot::save(&picture, &request).map_err(Failure::Unwritable)
        });
        cx.spawn(async move |this, cx| {
            let saved = saving.await;
            this.update(cx, |this, cx| this.snapshot_done(saved, cx)).ok();
        })
        .detach();
    }

    /// The picture on screen, and what to call it and where and how to save it.
    fn snapshot_request(&self, cx: &App) -> Option<(Still, Request)> {
        let still = self.video.as_ref()?.read(cx).still(cx)?;
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        let title = media_title(player.metadata().title.as_deref(), player.media()?);
        let settings = settings::get(cx);
        let request = Request {
            title,
            position: player.position(),
            format: settings.prefs.video.snapshot_format,
            folder: settings.local.snapshots_folder.clone(),
        };
        Some((still, request))
    }

    /// Shows what became of a snapshot.
    fn snapshot_done(&mut self, saved: Result<Saved, Failure>, cx: &mut Context<Self>) {
        let toast = match saved {
            Ok(saved) => saved_toast(saved),
            Err(failure) => failure_toast(&failure),
        };
        self.show_toast(toast, cx);
    }
}

/// The toast for a snapshot that wasn't saved; the details go to the log.
fn failure_toast(failure: &Failure) -> Toast {
    let (error, message) = match failure {
        Failure::Unreadable(error) => {
            (error, tr!("Couldn't take a snapshot: this video can't be captured."))
        }
        Failure::Unwritable(error) => {
            (error, tr!("Couldn't save the snapshot: no folder could be written."))
        }
    };
    tracing::warn!("snapshot: {error:#}");
    Toast::new(Level::Error, message)
}

/// The toast for a saved snapshot: its thumbnail, where it went, and Open, Show in Finder and
/// Copy, which leave it up so they can be pressed one after another.
fn saved_toast(saved: Saved) -> Toast {
    let Saved { path, instead_of, thumbnail } = saved;
    let folder = path.parent().map_or_else(String::new, display_path);
    let (level, message) = match instead_of {
        None => (Level::Success, tr!("Snapshot saved in {folder}", folder)),
        Some(wanted) => (
            Level::Warning,
            tr!(
                "Couldn't save in {wanted}, so the snapshot is in {folder}",
                wanted = display_path(&wanted),
                folder
            ),
        ),
    };
    let thumbnail = Arc::new(RenderImage::new([image::Frame::new(thumbnail)]));
    let (open, reveal, copy) = (path.clone(), path.clone(), path);
    Toast::new(level, message)
        .thumbnail(thumbnail)
        .lasting_action(tr!("Open"), move |_, cx| cx.open_with_system(&open))
        .lasting_action(show_in_folder_label(), move |_, cx| cx.reveal_path(&reveal))
        .lasting_action(tr!("Copy"), move |_, cx| copy_image(copy.clone(), cx))
}

/// Puts the image file at `path` on the clipboard, read off the UI thread.
fn copy_image(path: PathBuf, cx: &mut App) {
    let format = match path.extension().and_then(|extension| extension.to_str()) {
        Some("jpg") => ImageFormat::Jpeg,
        _ => ImageFormat::Png,
    };
    let read = cx.background_spawn(async move { std::fs::read(&path) });
    cx.spawn(async move |cx| match read.await {
        Ok(bytes) => cx.update(|cx| {
            cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(format, bytes)));
        }),
        Err(error) => tracing::warn!("snapshot: couldn't copy it: {error}"),
    })
    .detach();
}

#[cfg(test)]
mod tests;
