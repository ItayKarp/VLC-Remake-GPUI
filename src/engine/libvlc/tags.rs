//! The libVLC tag reader (S4.3.1): a libVLC instance of its own, created on the thread that
//! first reads, which parses a file's headers and reads its tags, tracks, length and cover.
//!
//! libVLC finds the cover while parsing ("fetch local"): a picture inside the file, which it
//! saves in its art cache, or an image next to the file. With [`Lookup::Online`] it may also
//! look the cover up on the internet.

pub mod values;

use std::ffi::{CString, c_int};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use self::values::{TrackSummary, TrackType};
use super::api::{Instance, LibVlc, Media, MediaTrack};
use super::prepare::{PARSE_LOCAL, TRACK_VIDEO, parse_with, video_track};
use super::query::display_size;
use crate::db::metadata::{Metadata, Resolution};
use crate::engine::display_text::clean_tag;
use crate::engine::tags::{Lookup, Probe, Probed, TagReader};

/// The reader's instance never plays, so it needs no outputs.
const ARGUMENTS: [&str; 3] = ["--no-audio", "--no-video", "--no-sub-autodetect-file"];
/// How long one file may take to parse before it counts as unreadable.
const PARSE_TIMEOUT: Duration = Duration::from_secs(10);
/// `libvlc_media_fetch_local`: find the cover in the file or next to it.
const FETCH_LOCAL: c_int = 0x02;
/// `libvlc_media_fetch_network`: also look the cover up online.
const FETCH_NETWORK: c_int = 0x04;
/// `libvlc_track_audio`.
const TRACK_AUDIO: c_int = 0;

/// `libvlc_meta_t` values.
mod meta {
    use std::ffi::c_int;

    pub const TITLE: c_int = 0;
    pub const ARTIST: c_int = 1;
    pub const GENRE: c_int = 2;
    pub const ALBUM: c_int = 4;
    pub const TRACK_NUMBER: c_int = 5;
    pub const DATE: c_int = 8;
    pub const ARTWORK_URL: c_int = 15;
    pub const ALBUM_ARTIST: c_int = 23;
    pub const DISC_NUMBER: c_int = 24;
}

/// A [`TagReader`] on libVLC. libVLC is loaded the first time it reads.
#[derive(Default)]
pub struct LibVlcTags {
    state: State,
}

/// Whether the reader has its instance yet.
#[derive(Default)]
enum State {
    /// Nothing read yet.
    #[default]
    Unloaded,
    /// Ready to read.
    Loaded(Loaded),
    /// libVLC couldn't be loaded (logged once); nothing can be read.
    Failed,
}

impl TagReader for LibVlcTags {
    fn read(&mut self, path: &Path, lookup: Lookup) -> Probe {
        if matches!(self.state, State::Unloaded) {
            self.state = match Loaded::new() {
                Ok(loaded) => State::Loaded(loaded),
                Err(error) => {
                    tracing::warn!("library: no tag reader: {error:#}");
                    State::Failed
                }
            };
        }
        match &self.state {
            State::Loaded(loaded) => loaded
                .read(path, lookup)
                .map_or(Probe::Unreadable, |probed| Probe::Read(Box::new(probed))),
            State::Unloaded | State::Failed => Probe::Unavailable,
        }
    }
}

/// libVLC and the reader's instance.
struct Loaded {
    vlc: Arc<LibVlc>,
    instance: *mut Instance,
}

// SAFETY: the instance is only used by whoever owns the reader, one call at a time.
unsafe impl Send for Loaded {}

impl Loaded {
    fn new() -> anyhow::Result<Self> {
        let vlc = super::library()?;
        let instance = super::new_instance(&vlc, &ARGUMENTS)?;
        Ok(Self { vlc, instance })
    }

    /// Parses the file at `path` and reads it; `None` if it can't be parsed.
    fn read(&self, path: &Path, lookup: Lookup) -> Option<Probed> {
        let text = CString::new(path.to_string_lossy().into_owned()).ok()?;
        let api = &self.vlc.api;
        // SAFETY: the instance is live and `text` is NUL-terminated.
        let media = unsafe { (api.libvlc_media_new_path)(self.instance, text.as_ptr()) };
        if media.is_null() {
            return None;
        }
        let flags = match lookup {
            // The local fetch saves a picture inside the file to the art cache; whether a
            // folder's cover is used is the extractor's choice.
            Lookup::File | Lookup::Local => PARSE_LOCAL | FETCH_LOCAL,
            Lookup::Online => PARSE_LOCAL | FETCH_LOCAL | FETCH_NETWORK,
        };
        // SAFETY: `media` is a live media of this instance, released once below.
        unsafe {
            let probed = parse_with(&self.vlc, media, flags, PARSE_TIMEOUT)
                .then(|| self.probed(media, path));
            (api.libvlc_media_release)(media);
            probed
        }
    }

    /// What parsed `media` (from `path`) holds.
    ///
    /// # Safety
    ///
    /// `media` is a live, parsed media of this instance.
    unsafe fn probed(&self, media: *mut Media, path: &Path) -> Probed {
        // SAFETY: the caller's promise; `get_meta` returns a string we own, or NULL.
        let tag = |kind| unsafe {
            clean_tag(self.vlc.take_string((self.vlc.api.libvlc_media_get_meta)(media, kind)))
        };
        // SAFETY: the caller's promise.
        let tracks = unsafe { self.tracks(media) };
        let metadata = Metadata {
            kind: values::kind(&tracks),
            title: values::tag_title(tag(meta::TITLE), path),
            artist: tag(meta::ARTIST),
            album: tag(meta::ALBUM),
            album_artist: tag(meta::ALBUM_ARTIST),
            track_number: tag(meta::TRACK_NUMBER).as_deref().and_then(values::tag_number),
            disc_number: tag(meta::DISC_NUMBER).as_deref().and_then(values::tag_number),
            genre: tag(meta::GENRE),
            year: tag(meta::DATE).as_deref().and_then(values::tag_year),
            codecs: values::codecs(&tracks),
            resolution: values::resolution(&tracks),
        };
        // SAFETY: the caller's promise.
        let millis = unsafe { (self.vlc.api.libvlc_media_get_duration)(media) };
        let duration = u64::try_from(millis).ok().filter(|&ms| ms > 0).map(Duration::from_millis);
        let artwork = tag(meta::ARTWORK_URL).as_deref().and_then(values::artwork_path);
        Probed { metadata, duration, artwork }
    }

    /// The tracks of parsed `media`.
    ///
    /// # Safety
    ///
    /// `media` is a live, parsed media of this instance.
    unsafe fn tracks(&self, media: *mut Media) -> Vec<TrackSummary> {
        let api = &self.vlc.api;
        let mut list: *mut *mut MediaTrack = std::ptr::null_mut();
        // SAFETY: the caller's promise; `list` receives `count` tracks we release.
        let count = unsafe { (api.libvlc_media_tracks_get)(media, &raw mut list) };
        if list.is_null() {
            return Vec::new();
        }
        // SAFETY: libVLC returned `count` live tracks, released right after reading.
        unsafe {
            let tracks = std::slice::from_raw_parts(list, count as usize)
                .iter()
                .map(|&track| summary(&*track))
                .collect();
            (api.libvlc_media_tracks_release)(list, count);
            tracks
        }
    }
}

impl Drop for Loaded {
    fn drop(&mut self) {
        // SAFETY: the instance is live and released once, after every media of it.
        unsafe { (self.vlc.api.libvlc_release)(self.instance) };
    }
}

/// What the library keeps of `track`.
///
/// # Safety
///
/// A video track's `detail` is NULL or a live `libvlc_video_track_t`.
unsafe fn summary(track: &MediaTrack) -> TrackSummary {
    let kind = match track.kind {
        TRACK_VIDEO => TrackType::Video,
        TRACK_AUDIO => TrackType::Audio,
        _ => TrackType::Other,
    };
    // SAFETY: the caller's promise.
    let video = unsafe { video_track(track) };
    let size = video
        .and_then(display_size)
        .map(|size| Resolution { width: size.width, height: size.height });
    TrackSummary { kind, codec: track.codec, size }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::db::metadata::MediaKind;

    #[test]
    #[ignore = "needs libVLC and the S1.1 test media (see `libvlc/tests.rs`)"]
    fn reads_kinds_sizes_codecs_and_lengths() {
        // SAFETY: the ignored libVLC tests run on one thread (`--test-threads 1`).
        unsafe { crate::engine::libvlc::runtime::prepare_environment() };
        let media = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media");
        let mut reader = LibVlcTags::default();
        let Probe::Read(probed) = reader.read(&media.join("1080p60-h264.mp4"), Lookup::Local)
        else {
            panic!("the test video reads");
        };
        assert_eq!(probed.metadata.kind, MediaKind::Video);
        assert_eq!(probed.metadata.resolution, Some(Resolution { width: 1920, height: 1080 }));
        assert!(probed.metadata.codecs.is_some_and(|codecs| codecs.starts_with("H.264")));
        assert_eq!(probed.duration, Some(Duration::from_secs(70)));
        let Probe::Read(song) = reader.read(&media.join("audio-only.flac"), Lookup::Local) else {
            panic!("the test song reads");
        };
        assert_eq!((song.metadata.kind, song.metadata.resolution), (MediaKind::Audio, None));
        let missing = reader.read(&media.join("no-such-file.mp4"), Lookup::Local);
        assert_eq!(missing, Probe::Unreadable);
    }
}
