//! The library (Stage 4): its model — what a media folder's paths compare as ([`path_key`]) and
//! how far each media was watched ([`watch_state`]) — and the indexer that fills its tables in
//! [`db`](crate::db) from the media folders (S4.2).
//!
//! [`Library`] is the GPUI global. [`init`] starts the [`indexer`] thread and keeps it on the
//! media folders the settings name ([`sources`]); windows observe the global for each folder's
//! status and scan progress.
//!
//! - [`walk`], [`recognize`], [`placeholder`]: finding media files in a folder tree.
//! - [`apply`], [`identify`]: writing what was found, and recognizing moved and copied files.
//! - [`watch`], [`root_check`], [`progress`]: live updates, folder status, scan progress.

pub mod apply;
pub mod identify;
pub mod indexer;
pub mod path_key;
pub mod placeholder;
pub mod progress;
pub mod recognize;
pub mod root_check;
pub mod sources;
pub mod walk;
pub mod watch;
#[allow(dead_code, reason = "the library's model (S4.1); the views (S4.6) are its first callers")]
pub mod watch_state;

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, AppContext as _, BorrowAppContext as _, Global};

use self::indexer::{Command, RootState, Wanted};
use crate::history::History;
use crate::opening::Cancel;
use crate::settings::{self, SettingsStore};

/// The library service: the indexer's channel and what it last reported.
pub struct Library {
    commands: Sender<Command>,
    roots: Vec<RootState>,
    /// What the indexer was last asked to index, to send only changes.
    wanted: Vec<Wanted>,
    show_hidden: Option<bool>,
}

impl Global for Library {}

/// Starts the indexer on the history thread's database and keeps it on the settings' media
/// folders. Without history (tests), there is no library.
pub fn init(cx: &mut App) {
    let Some(history) = History::get(cx) else { return };
    let (reports, mut received) = mpsc::unbounded();
    let cancel = Cancel::default();
    let report = Box::new(move |roots| {
        let _ = reports.unbounded_send(roots);
    });
    let commands = indexer::spawn(Box::new(history.database()), report, cancel.clone());
    cx.set_global(Library { commands, roots: Vec::new(), wanted: Vec::new(), show_hidden: None });
    cx.spawn(async move |cx| {
        while let Some(roots) = received.next().await {
            cx.update(|cx| cx.update_global::<Library, _>(|library, _| library.roots = roots));
        }
    })
    .detach();
    cx.observe_global::<SettingsStore>(follow_settings).detach();
    cx.on_app_quit(move |_| {
        cancel.cancel();
        async {}
    })
    .detach();
    follow_settings(cx);
}

/// Every media folder's state, onboarding's first.
pub fn roots(cx: &App) -> &[RootState] {
    cx.try_global::<Library>().map_or(&[], |library| &library.roots)
}

/// Resync (S4.2.5): rescans `folder` and everything below it, reading every file again.
pub fn resync(folder: &Path, cx: &App) {
    send(cx, Command::Resync(folder.to_path_buf()));
}

/// Replaces the media folder `old` with `new` (the onboarding folder was missing, and the user
/// chose another).
pub fn replace_media_folder(old: &Path, new: PathBuf, cx: &mut App) {
    settings::update(cx, |settings| {
        let folders = &mut settings.local.media_folders;
        match folders.iter_mut().find(|folder| *folder == old) {
            Some(folder) => *folder = new,
            None => folders.push(new),
        }
    });
}

/// Sends the indexer what the settings ask for now, if it changed, and offers the Music folder
/// once onboarding is done (S4.2.7).
fn follow_settings(cx: &mut App) {
    let settings = settings::get(cx);
    let wanted = sources::wanted(&settings.local);
    let show_hidden = settings.prefs.library.show_hidden_files;
    let offer_music = sources::should_offer_music(&settings.local);
    let Some(library) = cx.try_global::<Library>() else { return };
    let (old_wanted, old_hidden) = (library.wanted.clone(), library.show_hidden);
    if old_hidden != Some(show_hidden) {
        send(cx, Command::ShowHidden(show_hidden));
    }
    if old_wanted != wanted {
        send(cx, Command::SetRoots(wanted.clone()));
    }
    cx.update_global::<Library, _>(|library, _| {
        library.wanted = wanted;
        library.show_hidden = Some(show_hidden);
    });
    if offer_music {
        // Marked first, so the settings change this causes doesn't offer it again.
        settings::update(cx, |s| sources::add_music_folder(&mut s.local, None));
        let music = cx.background_spawn(async { sources::music_folder() });
        cx.spawn(async move |cx| {
            let Some(music) = music.await else { return };
            cx.update(|cx| {
                settings::update(cx, |s| sources::add_music_folder(&mut s.local, Some(&music)));
            });
        })
        .detach();
    }
}

fn send(cx: &App, command: Command) {
    let Some(library) = cx.try_global::<Library>() else { return };
    if library.commands.send(command).is_err() {
        tracing::warn!("library: the indexer is gone");
    }
}
