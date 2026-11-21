//! The libVLC 3 C API the engine calls, loaded at run time with `libloading` (ADR 0001), so
//! building, linting and CI never need libVLC installed.
//!
//! Every type here mirrors a libVLC 3.0 header declaration field for field.

use std::ffi::{CStr, c_char, c_float, c_int, c_uint, c_void};

use anyhow::{Context as _, Result};
use libloading::Library;

use super::runtime::Runtime;

/// Declares an opaque libVLC handle type.
macro_rules! opaque {
    ($($(#[$doc:meta])* $name:ident;)*) => {
        $($(#[$doc])* #[repr(C)] pub struct $name { _private: [u8; 0] })*
    };
}

opaque! {
    /// `libvlc_instance_t`.
    Instance;
    /// `libvlc_media_t`.
    Media;
    /// `libvlc_media_player_t`.
    MediaPlayer;
    /// `libvlc_event_manager_t`.
    EventManager;
}

/// `libvlc_event_t`: the event type, its sender, and a union of per-type payloads.
#[repr(C)]
pub struct Event {
    /// One of the `libvlc_event_e` values in [`super::events`].
    pub kind: c_int,
    /// The object that sent it.
    pub object: *mut c_void,
    /// The payload; which member is valid depends on `kind`.
    pub data: EventData,
}

/// The `u` union of `libvlc_event_t`, reduced to the member shapes the engine reads. All of them
/// start at offset 0, so reading the one matching the event type is sound.
#[repr(C)]
#[derive(Clone, Copy)]
pub union EventData {
    /// `new_cache` (buffering) and `new_position`.
    pub float: c_float,
    /// `new_time` and `new_length`, in milliseconds.
    pub time: i64,
    /// `new_seekable`, `new_pausable`, `new_count`, `new_scrambled`.
    pub int: c_int,
}

/// `libvlc_callback_t`.
pub type Callback = unsafe extern "C" fn(*const Event, *mut c_void);

/// `libvlc_media_track_t` from libVLC 3, field for field.
#[repr(C)]
pub struct MediaTrack {
    pub codec: u32,
    pub original_fourcc: u32,
    pub id: c_int,
    /// `libvlc_track_type_t`: -1 unknown, 0 audio, 1 video, 2 text.
    pub kind: c_int,
    pub profile: c_int,
    pub level: c_int,
    /// Points to a [`VideoTrack`] when `kind` is video.
    pub detail: *mut VideoTrack,
    pub bitrate: c_uint,
    pub language: *mut c_char,
    pub description: *mut c_char,
}

/// `libvlc_video_track_t` from libVLC 3, field for field.
#[repr(C)]
pub struct VideoTrack {
    pub height: c_uint,
    pub width: c_uint,
    pub sar_num: c_uint,
    pub sar_den: c_uint,
    pub frame_rate_num: c_uint,
    pub frame_rate_den: c_uint,
    pub orientation: c_int,
    pub projection: c_int,
    pub pose: [c_float; 4],
}

/// `libvlc_track_description_t`: one node of a linked list of track names.
#[repr(C)]
pub struct TrackDescription {
    pub id: c_int,
    pub name: *mut c_char,
    pub next: *mut TrackDescription,
}

/// `libvlc_chapter_description_t`.
#[repr(C)]
pub struct ChapterDescription {
    /// Start, in milliseconds.
    pub time_offset: i64,
    /// Length, in milliseconds.
    pub duration: i64,
    pub name: *mut c_char,
}

type Player = *mut MediaPlayer;

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
    libvlc_free: fn(*mut c_void);
    libvlc_event_attach: fn(*mut EventManager, c_int, Callback, *mut c_void) -> c_int;
    libvlc_event_detach: fn(*mut EventManager, c_int, Callback, *mut c_void);
    libvlc_media_new_path: fn(*mut Instance, *const c_char) -> *mut Media;
    libvlc_media_new_location: fn(*mut Instance, *const c_char) -> *mut Media;
    libvlc_media_release: fn(*mut Media);
    libvlc_media_event_manager: fn(*mut Media) -> *mut EventManager;
    libvlc_media_get_meta: fn(*mut Media, c_int) -> *mut c_char;
    libvlc_media_tracks_get: fn(*mut Media, *mut *mut *mut MediaTrack) -> c_uint;
    libvlc_media_tracks_release: fn(*mut *mut MediaTrack, c_uint);
    libvlc_media_player_new: fn(*mut Instance) -> Player;
    libvlc_media_player_release: fn(Player);
    libvlc_media_player_set_media: fn(Player, *mut Media);
    libvlc_media_player_event_manager: fn(Player) -> *mut EventManager;
    libvlc_media_player_play: fn(Player) -> c_int;
    libvlc_media_player_set_pause: fn(Player, c_int);
    libvlc_media_player_stop: fn(Player);
    libvlc_media_player_set_time: fn(Player, i64);
    libvlc_media_player_set_position: fn(Player, c_float);
    libvlc_media_player_get_time: fn(Player) -> i64;
    libvlc_media_player_next_frame: fn(Player);
    libvlc_media_player_get_fps: fn(Player) -> c_float;
    libvlc_media_player_set_rate: fn(Player, c_float) -> c_int;
    libvlc_media_player_get_full_chapter_descriptions:
        fn(Player, c_int, *mut *mut *mut ChapterDescription) -> c_int;
    libvlc_chapter_descriptions_release: fn(*mut *mut ChapterDescription, c_uint);
    libvlc_audio_set_volume: fn(Player, c_int) -> c_int;
    libvlc_audio_set_mute: fn(Player, c_int);
    libvlc_audio_get_track: fn(Player) -> c_int;
    libvlc_audio_set_track: fn(Player, c_int) -> c_int;
    libvlc_audio_get_track_description: fn(Player) -> *mut TrackDescription;
    libvlc_audio_set_delay: fn(Player, i64) -> c_int;
    libvlc_video_get_track: fn(Player) -> c_int;
    libvlc_video_set_track: fn(Player, c_int) -> c_int;
    libvlc_video_get_track_description: fn(Player) -> *mut TrackDescription;
    libvlc_video_get_spu: fn(Player) -> c_int;
    libvlc_video_set_spu: fn(Player, c_int) -> c_int;
    libvlc_video_get_spu_description: fn(Player) -> *mut TrackDescription;
    libvlc_video_set_spu_delay: fn(Player, i64) -> c_int;
    libvlc_media_player_add_slave: fn(Player, c_int, *const c_char, bool) -> c_int;
    libvlc_track_description_list_release: fn(*mut TrackDescription);
}

/// The libVLC libraries, core first, by their file names on this OS.
#[cfg(target_os = "macos")]
pub const LIBRARIES: [&str; 2] = ["libvlccore.dylib", "libvlc.dylib"];
#[cfg(target_os = "windows")]
pub const LIBRARIES: [&str; 2] = ["libvlccore.dll", "libvlc.dll"];
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const LIBRARIES: [&str; 2] = ["libvlccore.so.9", "libvlc.so.5"];

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
        unsafe { borrowed_string((self.api.libvlc_get_version)()) }.unwrap_or_default()
    }

    /// libVLC's last error message on this thread, for logs.
    pub fn last_error(&self) -> String {
        // SAFETY: libVLC returns NULL or a NUL-terminated string owned by the thread.
        unsafe { borrowed_string((self.api.libvlc_errmsg)()) }.unwrap_or_default()
    }

    /// Copies a string libVLC allocated for the caller, then frees it.
    ///
    /// # Safety
    ///
    /// `text` is NULL or a NUL-terminated string the caller owns and must `libvlc_free`.
    pub unsafe fn take_string(&self, text: *mut c_char) -> Option<String> {
        // SAFETY: the caller promises a NUL-terminated string or NULL.
        let copy = unsafe { borrowed_string(text) };
        if !text.is_null() {
            // SAFETY: the caller owns `text`, and it is freed exactly once.
            unsafe { (self.api.libvlc_free)(text.cast()) };
        }
        copy
    }
}

/// Copies a C string, treating NULL and empty as `None`.
///
/// # Safety
///
/// `text` is NULL or points to a NUL-terminated string.
pub unsafe fn borrowed_string(text: *const c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    // SAFETY: the caller promises a NUL-terminated string.
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    (!text.is_empty()).then(|| text.into_owned())
}
