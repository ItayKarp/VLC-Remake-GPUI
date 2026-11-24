//! The libVLC 3 C records the API passes by pointer (tracks, descriptions, devices,
//! statistics), each mirroring its libVLC 3.0 header declaration field for field.

use std::ffi::{c_char, c_float, c_int, c_uint, c_void};

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

/// `libvlc_media_stats_t` from libVLC 3, field for field. Bit rates are in bytes per
/// microsecond.
#[repr(C)]
#[derive(Default)]
pub struct MediaStats {
    pub read_bytes: c_int,
    pub input_bitrate: c_float,
    pub demux_read_bytes: c_int,
    pub demux_bitrate: c_float,
    pub demux_corrupted: c_int,
    pub demux_discontinuity: c_int,
    pub decoded_video: c_int,
    pub decoded_audio: c_int,
    pub displayed_pictures: c_int,
    pub lost_pictures: c_int,
    pub played_abuffers: c_int,
    pub lost_abuffers: c_int,
    pub sent_packets: c_int,
    pub sent_bytes: c_int,
    pub send_bitrate: c_float,
}

/// `libvlc_audio_output_device_t`: one node of a linked list of output devices.
#[repr(C)]
pub struct AudioOutputDevice {
    pub next: *mut AudioOutputDevice,
    pub device: *mut c_char,
    pub description: *mut c_char,
}
