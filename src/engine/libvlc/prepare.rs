//! Reads a local file's video geometry before it plays (S1.4.1, ADR 0001 limits 1–3).
//!
//! `vmem` proposes the decoder's coded size (1920×1088 for most 1080p H.264) in storage pixels.
//! Knowing the display size first lets the sink ask for it, so libVLC crops the padding and
//! squares anamorphic pixels itself. And libVLC 3 can only rotate software-decoded pictures, so
//! a rotated track is opened with hardware decoding off, or no picture would appear.
//!
//! Network media is not parsed (it could take seconds); it plays at the proposed size.

use std::ffi::c_int;
use std::thread;
use std::time::{Duration, Instant};

use super::api::{LibVlc, Media, MediaTrack, VideoTrack};
use super::decoding::{Decoding, SOFTWARE_DECODING};
use super::query::display_size;
use super::worker::Worker;
use crate::app::media::Media as Source;
use crate::engine::media_info::VideoSize;

/// `libvlc_media_parse_local`: read the file's headers, no network.
pub(super) const PARSE_LOCAL: c_int = 0;
/// `libvlc_media_parsed_status_done`; the statuses below it are "not yet", above it none.
const PARSED_DONE: c_int = 4;
/// `libvlc_media_parsed_status_skipped`, `_failed` and `_timeout`: parsing gave up.
const PARSE_GAVE_UP: [c_int; 3] = [1, 2, 3];
/// How long a local file may take to parse before it plays without the geometry.
const PARSE_TIMEOUT: Duration = Duration::from_millis(1500);
/// How often the worker checks whether parsing finished.
const PARSE_POLL: Duration = Duration::from_millis(5);
/// `libvlc_track_video`.
pub(super) const TRACK_VIDEO: c_int = 1;
/// `libvlc_video_orient_top_left`: stored upright. Every other orientation needs the
/// `transform` filter (flips, 180°, and the 90° turns that swap width and height).
const UPRIGHT: c_int = 0;

/// Parses a local file's headers, waiting until done or `timeout`; `false` if parsing failed or
/// took too long. Blocks the calling thread.
///
/// # Safety
///
/// `media` is a live `libvlc_media_t` of `vlc`.
pub(super) unsafe fn parse_headers(vlc: &LibVlc, media: *mut Media, timeout: Duration) -> bool {
    // SAFETY: the caller's promise.
    unsafe { parse_with(vlc, media, PARSE_LOCAL, timeout) }
}

/// Parses `media` with libVLC's `flags` (`libvlc_media_parse_flag_t`), waiting until done or
/// `timeout`; `false` if parsing failed or took too long. Blocks the calling thread.
///
/// # Safety
///
/// `media` is a live `libvlc_media_t` of `vlc`.
pub(super) unsafe fn parse_with(
    vlc: &LibVlc,
    media: *mut Media,
    flags: c_int,
    timeout: Duration,
) -> bool {
    let api = &vlc.api;
    let millis = c_int::try_from(timeout.as_millis()).unwrap_or(c_int::MAX);
    // SAFETY: the caller's promise.
    if unsafe { (api.libvlc_media_parse_with_options)(media, flags, millis) } != 0 {
        return false;
    }
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        // SAFETY: the caller's promise.
        let status = unsafe { (api.libvlc_media_get_parsed_status)(media) };
        if status == PARSED_DONE {
            return true;
        }
        if PARSE_GAVE_UP.contains(&status) {
            return false;
        }
        thread::sleep(PARSE_POLL);
    }
    false
}

/// How the first video track is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    display: VideoSize,
    rotated: bool,
}

impl Worker {
    /// Tells the video sink the display size of `media` (a local `source` only), and turns
    /// hardware decoding off when the track is rotated; returns how it will be decoded. Blocks
    /// the worker for the parse, at most [`PARSE_TIMEOUT`].
    pub(super) fn prepare_video(&self, media: *mut Media, source: &Source) -> Decoding {
        let Some(video) = &self.video else { return Decoding::Hardware };
        let geometry = match source {
            Source::File(_) if self.parse(media) => self.geometry(media),
            _ => None,
        };
        video.set_display(geometry.map(|geometry| geometry.display));
        if !geometry.is_some_and(|geometry| geometry.rotated) {
            return Decoding::Hardware;
        }
        tracing::info!("libvlc: rotated video; decoding in software so it can be rotated");
        self.add_option(media, &SOFTWARE_DECODING);
        Decoding::Software
    }

    /// Parses `media`'s headers, waiting until done; `false` if it failed or took too long.
    fn parse(&self, media: *mut Media) -> bool {
        // SAFETY: the media is live.
        unsafe { parse_headers(&self.vlc, media, PARSE_TIMEOUT) }
    }

    fn geometry(&self, media: *mut Media) -> Option<Geometry> {
        let api = &self.vlc.api;
        let mut list: *mut *mut MediaTrack = std::ptr::null_mut();
        // SAFETY: the media is live; `list` receives `count` tracks we release.
        let count = unsafe { (api.libvlc_media_tracks_get)(media, &raw mut list) };
        if list.is_null() {
            return None;
        }
        // SAFETY: libVLC returned `count` live tracks, released right after reading.
        unsafe {
            let geometry = std::slice::from_raw_parts(list, count as usize)
                .iter()
                .find_map(|&track| video_track(&*track).and_then(geometry));
            (api.libvlc_media_tracks_release)(list, count);
            geometry
        }
    }
}

/// The video detail of `track`, if it is a video track.
///
/// # Safety
///
/// A video track's `detail` is NULL or a live `libvlc_video_track_t`.
pub(super) unsafe fn video_track(track: &MediaTrack) -> Option<&VideoTrack> {
    if track.kind != TRACK_VIDEO {
        return None;
    }
    // SAFETY: the caller's promise.
    unsafe { track.detail.video.as_ref() }
}

fn geometry(video: &VideoTrack) -> Option<Geometry> {
    Some(Geometry { display: display_size(video)?, rotated: video.orientation != UPRIGHT })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(width: u32, height: u32, orientation: c_int) -> VideoTrack {
        VideoTrack {
            height,
            width,
            sar_num: 1,
            sar_den: 1,
            frame_rate_num: 0,
            frame_rate_den: 0,
            orientation,
            projection: 0,
            pose: [0.0; 4],
        }
    }

    #[test]
    fn rotated_tracks_are_flagged_and_their_sides_swapped() {
        let rotated = geometry(&video(1920, 1080, 5)).expect("known size");
        assert!(rotated.rotated);
        assert_eq!(rotated.display, VideoSize { width: 1080, height: 1920 });
        assert!(geometry(&video(1920, 1080, 3)).expect("upside down").rotated);
        assert!(!geometry(&video(1920, 1080, 0)).expect("known size").rotated);
    }

    #[test]
    fn unknown_sizes_give_no_geometry() {
        assert_eq!(geometry(&video(0, 0, 0)), None);
    }
}
