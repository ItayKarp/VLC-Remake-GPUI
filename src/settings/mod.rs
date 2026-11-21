//! Typed, crash-safe settings (S0.1).
//!
//! Settings load once at startup into the [`SettingsStore`] GPUI global. Read them with
//! [`get`], change them with [`update`]; views that should react call
//! `cx.observe_global::<SettingsStore>(…)`. Changes are saved after a short debounce on
//! a background thread, and flushed synchronously when the app quits.
//!
//! Files: `settings.toml` (roaming preferences) and `local.toml` (machine-local state),
//! see [`Locations`]. `RVLC_CONFIG_DIR` puts both in one directory.

mod document;
mod lenient;
mod migrate;
mod paths;
mod schema;
mod storage;
mod store;
mod stored_path;
#[cfg(test)]
mod tests;

use std::time::Duration;

use gpui::{App, AppContext, BorrowAppContext, Global, Task};

pub use paths::{Locations, default_media_folder, display_path};
#[allow(unused_imports)]
pub use schema::{LocalState, Preferences, Settings};
#[allow(unused_imports)]
pub use store::Notice;
use store::SettingsState;

/// How long to wait after the last change before writing to disk.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(500);

pub struct SettingsStore {
    state: SettingsState,
    /// The pending debounced save. Replacing it cancels the previous one.
    pending_save: Option<Task<()>>,
}

impl Global for SettingsStore {}

/// Loads settings and installs the global. Call once, before opening any window.
pub fn init(cx: &mut App) {
    let state = SettingsState::load(Locations::resolve());
    let needs_save = state.is_dirty(); // e.g. just migrated
    cx.set_global(SettingsStore {
        state,
        pending_save: None,
    });
    cx.on_app_quit(|cx| {
        if let Err(err) = flush(cx) {
            log::error!("settings: couldn't save on quit: {err}");
        }
        async {}
    })
    .detach();
    if needs_save {
        schedule_save(cx);
    }
}

pub fn get(cx: &App) -> &Settings {
    &cx.global::<SettingsStore>().state.settings
}

/// Changes settings and schedules a save if anything changed.
pub fn update(cx: &mut App, f: impl FnOnce(&mut Settings)) {
    let changed = cx.update_global::<SettingsStore, _>(|store, _| store.state.update(f));
    if changed {
        schedule_save(cx);
    }
}

/// Writes pending changes now, on this thread. Used on quit and where the caller
/// needs to know the result (finishing onboarding).
pub fn flush(cx: &mut App) -> Result<(), String> {
    let job = cx.update_global::<SettingsStore, _>(|store, _| {
        store.pending_save = None;
        store.state.prepare_save()
    });
    let Some(job) = job else {
        return Ok(());
    };
    let generation = job.generation();
    let result = job.run();
    cx.update_global::<SettingsStore, _>(|store, _| {
        store.state.finish_save(generation, result.clone())
    });
    result
}

/// Notices for the user (corrupt file recovered, settings not saved, …), each
/// returned once.
#[allow(dead_code)] // Shown by the main window shell (S0.2/S0.3).
pub fn take_notices(cx: &mut App) -> Vec<Notice> {
    cx.update_global::<SettingsStore, _>(|store, _| store.state.take_notices())
}

/// Whether changes are being written to disk.
#[allow(dead_code)] // Drives the "changes won't be saved" banner (S0.3).
pub fn is_persistent(cx: &App) -> bool {
    cx.global::<SettingsStore>().state.is_persistent()
}

fn schedule_save(cx: &mut App) {
    let task = cx.spawn(async move |cx| {
        cx.background_executor().timer(SAVE_DEBOUNCE).await;
        let job = cx.update_global::<SettingsStore, _>(|store, _| store.state.prepare_save());
        let Some(job) = job else {
            return;
        };
        let generation = job.generation();
        let result = cx.background_spawn(async move { job.run() }).await;
        cx.update_global::<SettingsStore, _>(|store, _| {
            store.state.finish_save(generation, result)
        });
    });
    cx.update_global::<SettingsStore, _>(|store, _| store.pending_save = Some(task));
}
