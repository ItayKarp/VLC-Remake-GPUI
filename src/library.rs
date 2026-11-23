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
//! - [`watch`], [`root_check`], [`progress`], [`root_text`]: live updates, folder status, scan
//!   progress, and what a folder's row says about them.
//! - [`folders`], [`system_folders`]: adding and removing media folders in Settings (S5.9.3),
//!   and the OS folders never indexed.
//! - [`extract`], [`artwork_cache`], [`folder_art`]: reading each media's tags and cover (S4.3).
//! - [`episode`], [`display`], [`properties`]: what the views and Properties & Metadata show for
//!   a media's tags.
//! - [`folder_view`], [`music`]: what a library folder and Music's categories list (S4.6).
//! - [`favorites`], [`copy_in`]: the media and playlists marked as favorites, and copying
//!   files into a library folder (S4.8).
//! - [`thumbnails`], [`thumbnailer`], [`thumbnail_cache`]: the views' thumbnails (S4.4).
//! - [`home`], [`frequency`]: what Home lists, and the fading counts that rank it (S4.9).

pub mod apply;
pub mod artwork_cache;
pub mod copy_in;
pub mod display;
pub mod episode;
pub mod extract;
pub mod favorites;
pub mod folder_art;
pub mod folder_view;
pub mod folders;
pub mod frequency;
pub mod home;
pub mod identify;
pub mod indexer;
pub mod music;
pub mod path_key;
pub mod placeholder;
pub mod progress;
pub mod properties;
pub mod recognize;
pub mod root_check;
pub mod root_text;
pub mod search;
pub mod sort;
pub mod sources;
pub mod system_folders;
pub mod thumbnail_cache;
pub mod thumbnailer;
pub mod thumbnails;
pub mod walk;
pub mod watch;
pub mod watch_state;

use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, AppContext as _, BorrowAppContext as _, Global};

use self::artwork_cache::ArtworkCache;
use self::extract::Extractor;
use self::indexer::{Command, RootState, Wanted};
use crate::engine::tags::{self, Lookup};
use crate::history::History;
use crate::opening::Cancel;
use crate::settings::{self, SettingsStore};

/// The cover cache's folder in the cache dir.
const ARTWORK_DIR_NAME: &str = "artwork";

/// The library service: the indexer's channel and what it last reported.
pub struct Library {
    commands: Sender<Command>,
    roots: Vec<RootState>,
    /// What the indexer was last asked to index, to send only changes.
    wanted: Vec<Wanted>,
    show_hidden: Option<bool>,
    lookup: Option<Lookup>,
    /// Whether the indexer was last told the library is on.
    enabled: Option<bool>,
}

impl Global for Library {}

/// Starts the indexer on the history thread's database and keeps it on the settings' media
/// folders. Without history (tests), there is no library.
pub fn init(cx: &mut App) {
    favorites::init(cx);
    let Some(history) = History::get(cx) else { return };
    let (reports, mut received) = mpsc::unbounded();
    let cancel = Cancel::default();
    let report = Box::new(move |roots| {
        let _ = reports.unbounded_send(roots);
    });
    let cache = artwork_cache();
    let thumbnail_store = Box::new(history.database());
    let extractor = Extractor::new(tags::default_reader(), cache.clone());
    let commands = indexer::spawn(Box::new(history.database()), extractor, report, cancel.clone());
    thumbnails::init(thumbnail_store, cache, cancel.clone(), cx);
    cx.set_global(Library {
        commands,
        roots: Vec::new(),
        wanted: Vec::new(),
        show_hidden: None,
        lookup: None,
        enabled: None,
    });
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

/// The cover cache in the cache dir, which music mode (S8.1) shares; none without a cache dir.
pub fn artwork_cache() -> Option<ArtworkCache> {
    settings::cache_dir().map(|dir| ArtworkCache::new(dir.join(ARTWORK_DIR_NAME)))
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
/// once onboarding is done (S4.2.7). With the media library off (S5.9.4), the indexer is
/// suspended instead: the folders stay in the database until the library is on again.
fn follow_settings(cx: &mut App) {
    let settings = settings::get(cx);
    let enabled = settings.prefs.library.enabled;
    let wanted = sources::wanted(&settings.local);
    let show_hidden = settings.prefs.library.show_hidden_files;
    let lookup = sources::lookup(&settings.prefs);
    let offer_music = sources::should_offer_music(&settings.local);
    let Some(library) = cx.try_global::<Library>() else { return };
    let roots = roots_command(library, enabled, &wanted);
    if library.show_hidden != Some(show_hidden) {
        send(cx, Command::ShowHidden(show_hidden));
    }
    if library.lookup != Some(lookup) {
        send(cx, Command::Lookup(lookup));
    }
    if let Some(command) = roots {
        send(cx, command);
    }
    cx.update_global::<Library, _>(|library, _| {
        library.wanted = wanted;
        library.show_hidden = Some(show_hidden);
        library.lookup = Some(lookup);
        library.enabled = Some(enabled);
    });
    if offer_music {
        offer_music_folder(cx);
    }
}

/// What the indexer needs to hear about the media folders, if anything: suspend when the
/// library was turned off, and the folders when they changed or the library was turned on.
fn roots_command(library: &Library, enabled: bool, wanted: &[Wanted]) -> Option<Command> {
    match (enabled, library.enabled) {
        (false, Some(false)) => None,
        (false, _) => Some(Command::Suspend),
        (true, Some(true)) if library.wanted == wanted => None,
        (true, _) => Some(Command::SetRoots(wanted.to_vec())),
    }
}

/// Adds the OS Music folder to the library's music folders, once (S4.2.7).
fn offer_music_folder(cx: &mut App) {
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

/// Deletes what the library indexed, while it is off (S5.9.4): its media folders leave the
/// database, with their files. The settings keep the folders, so turning the library on again
/// indexes them anew.
pub fn delete_index(cx: &App) {
    if cx.try_global::<Library>().is_some_and(|library| library.enabled == Some(false)) {
        send(cx, Command::SetRoots(Vec::new()));
    }
}

/// Whether the media library is used (Settings → Library, S5.9.2): its views show and its
/// folders are indexed.
pub fn is_enabled(cx: &App) -> bool {
    settings::get(cx).prefs.library.enabled
}

fn send(cx: &App, command: Command) {
    let Some(library) = cx.try_global::<Library>() else { return };
    if library.commands.send(command).is_err() {
        tracing::warn!("library: the indexer is gone");
    }
}
