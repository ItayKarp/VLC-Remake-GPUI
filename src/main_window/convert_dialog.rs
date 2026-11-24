//! The Convert dialog (S8.10.1), not designed, in Open Disc's look
//! ([`crate::main_window::device_dialog`]): **Files** (what to convert, with Add Files…),
//! **Format** (the presets), **Destination** (next to each file, or a folder picked), and
//! **Conversions**, the app's queue with each one's progress and a button to cancel it.
//!
//! Start queues the files and keeps the dialog open on the queue; closing it leaves the
//! conversions running in the background (S8.10.2).

mod open;
mod queue_rows;
mod render;

use std::path::PathBuf;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    PathPromptOptions, SharedString, Subscription, Window,
};

use crate::app::media::Media;
use crate::convert::{Conversions, Destination, Preset};
use crate::ui::dropdown::Dropdown;

pub use open::ask_before_quitting;
pub(super) use open::may_close;

/// The Convert dialog's view.
pub(super) struct ConvertDialog {
    /// What Start converts.
    sources: Vec<Media>,
    presets: Entity<Dropdown>,
    destination: Destination,
    conversions: Entity<Conversions>,
    focus: FocusHandle,
    _subscriptions: [Subscription; 1],
}

impl EventEmitter<DismissEvent> for ConvertDialog {}

impl Focusable for ConvertDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl ConvertDialog {
    /// The dialog, holding `sources`, starting from the format and destination last asked for.
    pub(super) fn new(
        sources: Vec<Media>,
        conversions: Entity<Conversions>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (preset, destination) = conversions.read(cx).choice.clone();
        let selected = Preset::ALL.iter().position(|&each| each == preset).unwrap_or_default();
        let presets = cx.new(|cx| {
            let names = Preset::ALL.iter().map(|preset| SharedString::from(preset.name()));
            Dropdown::new(tr!("Format"), names.collect(), selected, window, cx)
        });
        let subscriptions = [cx.observe(&conversions, |_, _, cx| cx.notify())];
        Self {
            sources,
            presets,
            destination,
            conversions,
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// The format picked.
    fn preset(&self, cx: &App) -> Preset {
        Preset::ALL.get(self.presets.read(cx).selected()).copied().unwrap_or_default()
    }

    /// Asks for files and adds those not listed yet.
    fn add_files(window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr!("Add").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            this.update(cx, |this, cx| this.add_paths(paths, cx)).ok();
        })
        .detach();
    }

    /// Lists the files at `paths` not listed yet.
    fn add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for media in paths.into_iter().map(Media::File) {
            if !self.sources.contains(&media) {
                self.sources.push(media);
            }
        }
        cx.notify();
    }

    /// Takes source `index` off the list.
    fn remove_source(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.sources.len() {
            self.sources.remove(index);
            cx.notify();
        }
    }

    /// Puts conversions next to their files, or asks for a folder to put them in.
    fn set_destination(&mut self, in_folder: bool, window: &mut Window, cx: &mut Context<Self>) {
        if in_folder {
            Self::pick_folder(window, cx);
        } else {
            self.destination = Destination::NextToSource;
            cx.notify();
        }
    }

    /// Asks for the folder conversions go to.
    fn pick_folder(window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("Choose").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else { return };
            let Some(folder) = paths.pop() else { return };
            this.update(cx, |this, cx| this.use_folder(folder, cx)).ok();
        })
        .detach();
    }

    /// Puts conversions in `folder`.
    fn use_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        self.destination = Destination::Folder(folder);
        cx.notify();
    }

    /// Start: queues the files and empties the list, the dialog staying open on the queue.
    fn start(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.sources.is_empty() {
            return;
        }
        let sources = std::mem::take(&mut self.sources);
        let (preset, destination) = (self.preset(cx), self.destination.clone());
        self.conversions.update(cx, |conversions, cx| {
            conversions.add(sources, preset, destination, cx);
        });
        cx.notify();
    }
}
