//! Loads libVLC 3 at run time and exposes the few C entry points the spike calls.
//!
//! Loading the library at run time, instead of linking it, keeps `cargo build`, clippy and CI
//! working on machines without libVLC: only running the spike needs it.

use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use libloading::Library;

use crate::track::MediaTrackC;

/// An opaque `libvlc_instance_t`.
#[repr(C)]
pub struct Instance {
    _private: [u8; 0],
}

/// An opaque `libvlc_media_t`.
#[repr(C)]
pub struct Media {
    _private: [u8; 0],
}

/// An opaque `libvlc_media_player_t`.
#[repr(C)]
pub struct MediaPlayer {
    _private: [u8; 0],
}

/// `libvlc_media_stats_t` from libVLC 3, field for field.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MediaStats {
    pub read_bytes: c_int,
    pub input_bitrate: f32,
    pub demux_read_bytes: c_int,
    pub demux_bitrate: f32,
    pub demux_corrupted: c_int,
    pub demux_discontinuity: c_int,
    pub decoded_video: c_int,
    pub decoded_audio: c_int,
    /// Pictures the video output showed on time.
    pub displayed_pictures: c_int,
    /// Pictures the video output dropped because they were late.
    pub lost_pictures: c_int,
    pub played_abuffers: c_int,
    pub lost_abuffers: c_int,
    pub sent_packets: c_int,
    pub sent_bytes: c_int,
    pub send_bitrate: f32,
}

/// `libvlc_video_lock_cb`: hands libVLC the plane pointers for the next picture.
pub type LockCb = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void;
/// `libvlc_video_unlock_cb`: libVLC finished writing the picture.
pub type UnlockCb = unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void);
/// `libvlc_video_display_cb`: the picture is due on screen now.
pub type DisplayCb = unsafe extern "C" fn(*mut c_void, *mut c_void);
/// `libvlc_video_format_cb`: agrees on chroma, size, pitches and lines.
pub type FormatCb = unsafe extern "C" fn(
    *mut *mut c_void,
    *mut c_char,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
) -> c_uint;
/// `libvlc_video_cleanup_cb`: the video output is closing.
pub type CleanupCb = unsafe extern "C" fn(*mut c_void);

/// Declares [`Api`] with one function pointer per libVLC symbol, named exactly as in C.
macro_rules! libvlc_api {
    ($($name:ident: fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        /// The libVLC entry points, resolved from the loaded library.
        #[allow(non_snake_case, reason = "fields are named after the C symbols they hold")]
        pub struct Api {
            $(pub $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
        }

        impl Api {
            fn resolve(library: &Library) -> Result<Self> {
                // SAFETY: each symbol is declared above with its libVLC 3 C signature.
                unsafe {
                    Ok(Self {
                        $($name: *library
                            .get(concat!(stringify!($name), "\0").as_bytes())
                            .context(stringify!($name))?,)*
                    })
                }
            }
        }
    };
}

libvlc_api! {
    libvlc_new: fn(c_int, *const *const c_char) -> *mut Instance;
    libvlc_release: fn(*mut Instance);
    libvlc_errmsg: fn() -> *const c_char;
    libvlc_get_version: fn() -> *const c_char;
    libvlc_media_new_path: fn(*mut Instance, *const c_char) -> *mut Media;
    libvlc_media_release: fn(*mut Media);
    libvlc_media_get_stats: fn(*mut Media, *mut MediaStats) -> c_int;
    libvlc_media_parse: fn(*mut Media);
    libvlc_media_tracks_get: fn(*mut Media, *mut *mut *mut MediaTrackC) -> c_uint;
    libvlc_media_tracks_release: fn(*mut *mut MediaTrackC, c_uint);
    libvlc_media_player_new_from_media: fn(*mut Media) -> *mut MediaPlayer;
    libvlc_media_player_release: fn(*mut MediaPlayer);
    libvlc_media_player_play: fn(*mut MediaPlayer) -> c_int;
    libvlc_media_player_stop: fn(*mut MediaPlayer);
    libvlc_media_player_get_time: fn(*mut MediaPlayer) -> i64;
    libvlc_video_set_callbacks:
        fn(*mut MediaPlayer, LockCb, Option<UnlockCb>, Option<DisplayCb>, *mut c_void);
    libvlc_video_set_format_callbacks: fn(*mut MediaPlayer, FormatCb, Option<CleanupCb>);
}

/// Where the libVLC runtime lives: the libraries and the `plugins/` folder.
#[derive(Clone, Debug)]
pub struct Runtime {
    /// Folder holding `libvlc` and `libvlccore`, or `None` for the system search path.
    pub lib_dir: Option<PathBuf>,
    /// Folder holding the plugins, exported as `VLC_PLUGIN_PATH` when set.
    pub plugin_dir: Option<PathBuf>,
}

impl Runtime {
    /// The layout of an unpacked VLC under `root`, per OS (see the S1.1.4 shipping notes).
    pub fn under(root: &Path) -> Self {
        let lib_dir = if cfg!(target_os = "macos") { root.join("lib") } else { root.to_owned() };
        Self { lib_dir: Some(lib_dir), plugin_dir: Some(root.join("plugins")) }
    }
}

#[cfg(target_os = "macos")]
const LIBRARIES: [&str; 2] = ["libvlccore.dylib", "libvlc.dylib"];
#[cfg(target_os = "windows")]
const LIBRARIES: [&str; 2] = ["libvlccore.dll", "libvlc.dll"];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const LIBRARIES: [&str; 2] = ["libvlccore.so.9", "libvlc.so.5"];

/// A loaded libVLC: its entry points, valid for as long as this value lives.
pub struct LibVlc {
    /// The resolved entry points.
    pub api: Api,
    _libraries: Vec<Library>,
}

impl LibVlc {
    /// Loads `libvlccore` then `libvlc` from `runtime` (core first, so `@rpath` resolves).
    pub fn load(runtime: &Runtime) -> Result<Self> {
        let libraries = LIBRARIES
            .iter()
            .map(|name| {
                let path = runtime.lib_dir.as_ref().map_or_else(|| name.into(), |d| d.join(name));
                // SAFETY: libVLC's initializers only set up its own globals.
                unsafe { Library::new(&path) }
                    .with_context(|| format!("loading {}", path.display()))
            })
            .collect::<Result<Vec<_>>>()?;
        let api = Api::resolve(&libraries[1])?;
        Ok(Self { api, _libraries: libraries })
    }

    /// The libVLC version string, such as `3.0.23 Vetinari`.
    pub fn version(&self) -> String {
        // SAFETY: libVLC returns a static NUL-terminated string.
        unsafe { c_string((self.api.libvlc_get_version)()) }
    }

    /// libVLC's last error message on this thread, for error reports.
    pub fn last_error(&self) -> String {
        // SAFETY: libVLC returns NULL or a NUL-terminated string owned by the thread.
        unsafe { c_string((self.api.libvlc_errmsg)()) }
    }
}

/// Copies a C string, treating NULL as empty.
///
/// # Safety
///
/// `text` is NULL or points to a NUL-terminated string.
unsafe fn c_string(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: the caller promises a NUL-terminated string.
    unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
}
