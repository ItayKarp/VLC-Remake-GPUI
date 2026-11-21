//! The OS recent documents list (S0.6.5): the Dock menu on macOS, the taskbar jump
//! list and Explorer's Recent on Windows. Skipped when the user turned off "Save
//! recently played items", because the OS list is history too (A.13).
//!
//! Linux has no GPUI hook for this, and the freedesktop `recently-used.xbel` list is
//! left alone for now.

use std::path::PathBuf;

use gpui::App;

use super::media::Media;
use crate::settings;

/// Adds the local files among `items` to the OS recent documents list, if the
/// privacy setting allows it. URLs aren't added: the OS lists only files.
pub fn add(items: &[Media], cx: &mut App) {
    if !settings::get(cx).prefs.privacy.save_recently_played {
        return;
    }
    let files: Vec<PathBuf> = items.iter().filter_map(Media::as_path).map(Into::into).collect();
    if !files.is_empty() {
        add_files(files, cx);
    }
}

/// GPUI implements the list on macOS (and as a no-op on Linux).
#[cfg(not(windows))]
fn add_files(files: Vec<PathBuf>, cx: &mut App) {
    for file in &files {
        cx.add_recent_document(file);
    }
}

/// GPUI doesn't implement it on Windows, so call the shell directly. The shell
/// writes a shortcut per file, so it runs off the UI thread.
#[cfg(windows)]
fn add_files(files: Vec<PathBuf>, _cx: &mut App) {
    let spawned = std::thread::Builder::new().name("recent-documents".into()).spawn(move || {
        windows_shell::add_to_recent_docs(&files);
    });
    if let Err(err) = spawned {
        tracing::warn!("couldn't update the recent documents list: {err}");
    }
}

#[cfg(windows)]
mod windows_shell {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    /// `SHARD_PATHW`: the argument is a null-terminated UTF-16 path.
    const SHARD_PATHW: u32 = 3;
    /// `COINIT_APARTMENTTHREADED`: the shell expects a single-threaded apartment.
    const COINIT_APARTMENTTHREADED: u32 = 2;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn SHAddToRecentDocs(flags: u32, path: *const c_void);
    }

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitializeEx(reserved: *const c_void, flags: u32) -> i32;
        fn CoUninitialize();
    }

    /// Adds each file to Explorer's Recent folder and the app's jump list.
    pub fn add_to_recent_docs(files: &[PathBuf]) {
        // SAFETY: called once on this fresh thread, balanced by CoUninitialize below.
        let initialized = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED) };
        for file in files {
            let wide: Vec<u16> = file.as_os_str().encode_wide().chain([0]).collect();
            // SAFETY: `wide` is a null-terminated UTF-16 string that outlives the call.
            unsafe { SHAddToRecentDocs(SHARD_PATHW, wide.as_ptr().cast()) };
        }
        if initialized >= 0 {
            // SAFETY: balances the successful CoInitializeEx above on the same thread.
            unsafe { CoUninitialize() };
        }
    }
}
