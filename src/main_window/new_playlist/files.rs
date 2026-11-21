//! Adding files to the New Playlist dialog: dropped on the drop zone or picked after clicking it.
//! They resolve in the background like any open (folders scanned, playlist files expanded), so
//! dropping a thousand files doesn't block the window; Save waits until they're in.

use gpui::{
    AppContext as _, Context, ExternalPaths, PathPromptOptions, SharedString, Task, Window,
};

use super::NewPlaylistDialog;
use crate::app::media::Media;
use crate::opening::resolve::{self, Options};
use crate::opening::scan::Subfolders;
use crate::opening::{Cancel, Resolved};
use crate::settings::{self, QueueSiblings};

/// The additions still resolving. Dropping it stops them.
#[derive(Default)]
pub(super) struct Adding {
    jobs: Vec<Job>,
    next_id: u64,
}

struct Job {
    id: u64,
    cancel: Cancel,
    _task: Task<()>,
}

impl Adding {
    /// Whether nothing is being added.
    pub(super) fn is_idle(&self) -> bool {
        self.jobs.is_empty()
    }

    /// Takes job `id` off the list; false when it was already gone.
    fn finish(&mut self, id: u64) -> bool {
        let before = self.jobs.len();
        self.jobs.retain(|job| job.id != id);
        self.jobs.len() != before
    }
}

impl Drop for Adding {
    fn drop(&mut self) {
        for job in &self.jobs {
            job.cancel.cancel();
        }
    }
}

impl NewPlaylistDialog {
    /// Files dropped on the drop zone.
    pub(super) fn drop_files(
        &mut self,
        paths: &ExternalPaths,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = paths.paths().iter().cloned().map(Media::from).collect();
        self.add(items, window, cx);
    }

    /// A click on the drop zone: asks for files and adds them.
    pub(super) fn pick_files(window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            this.update_in(cx, |this, window, cx| {
                this.add(paths.into_iter().map(Media::from).collect(), window, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Resolves `items` in the background, then adds what they hold.
    fn add(&mut self, items: Vec<Media>, window: &mut Window, cx: &mut Context<Self>) {
        if items.is_empty() {
            return;
        }
        let prefs = &settings::get(cx).prefs;
        let options = Options {
            subfolders: Subfolders::from_setting(prefs.library.include_subfolders),
            siblings: QueueSiblings::Never,
        };
        let cancel = Cancel::default();
        let job_cancel = cancel.clone();
        let id = self.adding.next_id;
        self.adding.next_id += 1;
        let task = cx.spawn_in(window, async move |this, cx| {
            let resolved =
                cx.background_spawn(async move { resolve::resolve(items, options, &job_cancel) });
            let resolved = resolved.await;
            this.update(cx, |this, cx| this.finish_adding(id, resolved, cx)).ok();
        });
        self.adding.jobs.push(Job { id, cancel, _task: task });
        self.note = None;
        cx.notify();
    }

    /// Adds what job `id` resolved to, and notes what it left out.
    fn finish_adding(&mut self, id: u64, resolved: Option<Resolved>, cx: &mut Context<Self>) {
        if !self.adding.finish(id) {
            return;
        }
        cx.notify();
        let Some(resolved) = resolved else { return };
        self.added_files |= !resolved.media.is_empty();
        self.note = left_out(&resolved);
        self.files.extend(resolved.media);
    }
}

/// What the drop zone says about files an addition couldn't take.
fn left_out(resolved: &Resolved) -> Option<SharedString> {
    let count = resolved.skipped + resolved.problems.len();
    match count {
        0 => None,
        1 => Some("1 item couldn't be added: it isn't media or couldn't be read.".into()),
        n => Some(
            format!("{n} items couldn't be added: they aren't media or couldn't be read.").into(),
        ),
    }
}
