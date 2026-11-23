//! The libVLC 3 C API the engine calls, loaded at run time with `libloading` (ADR 0001), so
//! building, linting and CI never need libVLC installed.
//!
//! Every type here mirrors a libVLC 3.0 header declaration field for field.

use std::ffi::{CStr, c_char, c_float, c_int, c_uint, c_void};

use anyhow::{Context as _, Result};
use libloading::Library;

use super::certificates::{DialogCallbacks, DialogId};
use super::core_vars::CoreVars;
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
    /// `libvlc_equalizer_t`.
    AudioEqualizer;
    /// `libvlc_media_list_t`.
    MediaList;
    /// `libvlc_renderer_discoverer_t`.
    RendererDiscoverer;
    /// `libvlc_renderer_item_t`.
    RendererItem;
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
    /// `renderer_discoverer_item_added.item` and `renderer_discoverer_item_deleted.item`.
    pub renderer: *mut RendererItem,
}

/// `libvlc_callback_t`.
pub type Callback = unsafe extern "C" fn(*const Event, *mut c_void);

/// `libvlc_video_format_cb`: agrees on the picture format (chroma, size, pitches, lines).
pub type FormatCallback = unsafe extern "C" fn(
    *mut *mut c_void,
    *mut c_char,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
    *mut c_uint,
) -> c_uint;
/// `libvlc_video_cleanup_cb`: the video output closed.
pub type CleanupCallback = unsafe extern "C" fn(*mut c_void);
/// `libvlc_video_lock_cb`: hands out the plane buffers for the next picture.
pub type LockCallback = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void;
/// `libvlc_video_unlock_cb`: the picture is written.
pub type UnlockCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *const *mut c_void);
/// `libvlc_video_display_cb`: the picture is due on screen.
pub type DisplayCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

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
    /// The kind's details; which member is valid depends on `kind`.
    pub detail: TrackDetail,
    pub bitrate: c_uint,
    pub language: *mut c_char,
    pub description: *mut c_char,
}

/// The anonymous union in `libvlc_media_track_t`: one pointer, to the details of the track's
/// kind. Each may be NULL.
#[repr(C)]
#[derive(Clone, Copy)]
pub union TrackDetail {
    /// `libvlc_audio_track_t`, when the track is audio.
    pub audio: *mut AudioTrack,
    /// `libvlc_video_track_t`, when the track is video.
    pub video: *mut VideoTrack,
    /// `libvlc_subtitle_track_t`, when the track is text; unread.
    pub subtitle: *mut c_void,
}

/// `libvlc_audio_track_t` from libVLC 3, field for field.
#[repr(C)]
pub struct AudioTrack {
    pub channels: c_uint,
    pub rate: c_uint,
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

/// `libvlc_title_description_t`.
#[repr(C)]
pub struct TitleDescription {
    /// Length, in milliseconds.
    pub duration: i64,
    pub name: *mut c_char,
    /// `libvlc_title_menu` (1) and `libvlc_title_interactive` (2).
    pub flags: c_uint,
}

/// `libvlc_rd_description_t`: a renderer discovery service.
#[repr(C)]
pub struct RendererService {
    pub name: *mut c_char,
    pub long_name: *mut c_char,
}

/// `libvlc_audio_output_device_t`: one node of a linked list of output devices.
#[repr(C)]
pub struct AudioOutputDevice {
    pub next: *mut AudioOutputDevice,
    pub device: *mut c_char,
    pub description: *mut c_char,
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
    libvlc_media_add_option: fn(*mut Media, *const c_char);
    libvlc_media_parse_with_options: fn(*mut Media, c_int, c_int) -> c_int;
    libvlc_media_get_parsed_status: fn(*mut Media) -> c_int;
    libvlc_media_get_duration: fn(*mut Media) -> i64;
    libvlc_media_event_manager: fn(*mut Media) -> *mut EventManager;
    libvlc_media_get_meta: fn(*mut Media, c_int) -> *mut c_char;
    libvlc_media_tracks_get: fn(*mut Media, *mut *mut *mut MediaTrack) -> c_uint;
    libvlc_media_tracks_release: fn(*mut *mut MediaTrack, c_uint);
    libvlc_media_slaves_add: fn(*mut Media, c_int, c_uint, *const c_char) -> c_int;
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
    libvlc_media_player_get_length: fn(Player) -> i64;
    libvlc_media_player_get_position: fn(Player) -> c_float;
    libvlc_media_player_next_frame: fn(Player);
    libvlc_media_player_get_fps: fn(Player) -> c_float;
    libvlc_media_player_set_rate: fn(Player, c_float) -> c_int;
    libvlc_media_player_get_full_chapter_descriptions:
        fn(Player, c_int, *mut *mut *mut ChapterDescription) -> c_int;
    libvlc_chapter_descriptions_release: fn(*mut *mut ChapterDescription, c_uint);
    libvlc_media_player_get_full_title_descriptions:
        fn(Player, *mut *mut *mut TitleDescription) -> c_int;
    libvlc_title_descriptions_release: fn(*mut *mut TitleDescription, c_uint);
    libvlc_media_player_get_title: fn(Player) -> c_int;
    libvlc_media_player_set_title: fn(Player, c_int);
    libvlc_media_player_navigate: fn(Player, c_uint);
    libvlc_media_subitems: fn(*mut Media) -> *mut MediaList;
    libvlc_media_list_lock: fn(*mut MediaList);
    libvlc_media_list_unlock: fn(*mut MediaList);
    libvlc_media_list_count: fn(*mut MediaList) -> c_int;
    libvlc_media_list_item_at_index: fn(*mut MediaList, c_int) -> *mut Media;
    libvlc_media_list_release: fn(*mut MediaList);
    libvlc_audio_set_volume: fn(Player, c_int) -> c_int;
    libvlc_audio_set_mute: fn(Player, c_int);
    libvlc_audio_get_mute: fn(Player) -> c_int;
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
    libvlc_audio_output_device_enum: fn(Player) -> *mut AudioOutputDevice;
    libvlc_audio_output_device_list_release: fn(*mut AudioOutputDevice);
    libvlc_audio_output_device_set: fn(Player, *const c_char, *const c_char);
    libvlc_audio_set_channel: fn(Player, c_int) -> c_int;
    libvlc_audio_get_channel: fn(Player) -> c_int;
    libvlc_audio_equalizer_new: fn() -> *mut AudioEqualizer;
    libvlc_audio_equalizer_new_from_preset: fn(c_uint) -> *mut AudioEqualizer;
    libvlc_audio_equalizer_release: fn(*mut AudioEqualizer);
    libvlc_audio_equalizer_set_preamp: fn(*mut AudioEqualizer, c_float) -> c_int;
    libvlc_audio_equalizer_get_preamp: fn(*mut AudioEqualizer) -> c_float;
    libvlc_audio_equalizer_set_amp_at_index: fn(*mut AudioEqualizer, c_float, c_uint) -> c_int;
    libvlc_audio_equalizer_get_amp_at_index: fn(*mut AudioEqualizer, c_uint) -> c_float;
    libvlc_audio_equalizer_get_preset_count: fn() -> c_uint;
    libvlc_media_player_set_equalizer: fn(Player, *mut AudioEqualizer) -> c_int;
    libvlc_video_set_adjust_int: fn(Player, c_uint, c_int);
    libvlc_video_set_adjust_float: fn(Player, c_uint, c_float);
    libvlc_video_set_callbacks: fn(
        Player,
        LockCallback,
        Option<UnlockCallback>,
        Option<DisplayCallback>,
        *mut c_void
    );
    libvlc_video_set_format_callbacks: fn(Player, FormatCallback, Option<CleanupCallback>);
    libvlc_dialog_set_callbacks: fn(*mut Instance, *const DialogCallbacks, *mut c_void);
    libvlc_dialog_post_action: fn(*mut DialogId, c_int) -> c_int;
    libvlc_dialog_dismiss: fn(*mut DialogId) -> c_int;
    libvlc_renderer_discoverer_list_get:
        fn(*mut Instance, *mut *mut *mut RendererService) -> usize;
    libvlc_renderer_discoverer_list_release: fn(*mut *mut RendererService, usize);
    libvlc_renderer_discoverer_new: fn(*mut Instance, *const c_char) -> *mut RendererDiscoverer;
    libvlc_renderer_discoverer_release: fn(*mut RendererDiscoverer);
    libvlc_renderer_discoverer_start: fn(*mut RendererDiscoverer) -> c_int;
    libvlc_renderer_discoverer_stop: fn(*mut RendererDiscoverer);
    libvlc_renderer_discoverer_event_manager: fn(*mut RendererDiscoverer) -> *mut EventManager;
    libvlc_renderer_item_hold: fn(*mut RendererItem) -> *mut RendererItem;
    libvlc_renderer_item_release: fn(*mut RendererItem);
    libvlc_renderer_item_name: fn(*const RendererItem) -> *const c_char;
    libvlc_renderer_item_flags: fn(*const RendererItem) -> c_int;
    libvlc_media_player_set_renderer: fn(Player, *mut RendererItem) -> c_int;
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
    /// The core's variable calls.
    pub(super) core: CoreVars,
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
        let core = CoreVars::resolve(&libraries[0])?;
        Ok(Self { api, core, _libraries: libraries })
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
