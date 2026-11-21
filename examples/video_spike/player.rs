//! One libVLC instance playing one file into a [`VideoSink`].

use std::ffi::{CString, c_char};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context as _, Result, bail};

use crate::libvlc::{Instance, LibVlc, Media, MediaPlayer, MediaStats};
use crate::sink::{self, SinkHandle, VideoSink};
use crate::track::{MediaTrackC, VideoGeometry};

/// A playing file. Dropping it stops playback and releases everything.
pub struct Player {
    vlc: Arc<LibVlc>,
    instance: *mut Instance,
    media: *mut Media,
    handle: *mut MediaPlayer,
    /// Freed in `drop` only after the player stopped: libVLC calls into the sink until then.
    sink: Option<SinkHandle>,
}

impl Player {
    /// Opens `path` with libVLC `arguments`. Call [`Self::attach`] before [`Self::play`].
    pub fn open(vlc: Arc<LibVlc>, path: &Path, arguments: &[String]) -> Result<Self> {
        let instance = new_instance(&vlc, arguments)?;
        let mut this = Self {
            vlc,
            instance,
            media: std::ptr::null_mut(),
            handle: std::ptr::null_mut(),
            sink: None,
        };
        let c_path = CString::new(path.to_string_lossy().as_bytes()).context("NUL in path")?;
        let api = &this.vlc.api;
        // SAFETY: the instance is live and the path is NUL-terminated.
        this.media = unsafe { (api.libvlc_media_new_path)(instance, c_path.as_ptr()) };
        if this.media.is_null() {
            bail!("libvlc_media_new_path: {}", this.vlc.last_error());
        }
        // SAFETY: the media is live; the player takes its own reference.
        this.handle = unsafe { (api.libvlc_media_player_new_from_media)(this.media) };
        if this.handle.is_null() {
            bail!("libvlc_media_player_new_from_media: {}", this.vlc.last_error());
        }
        Ok(this)
    }

    /// Sends the player's video into `sink`.
    pub fn attach(&mut self, sink: Arc<dyn VideoSink>) {
        self.sink = Some(sink::install(&self.vlc, self.handle, sink));
    }

    /// Parses the media's headers. This blocks; the real engine must parse asynchronously.
    pub fn parse(&self) {
        // SAFETY: the media is live.
        unsafe { (self.vlc.api.libvlc_media_parse)(self.media) };
    }

    /// The first video track's geometry, once parsing or playback has found one. Some
    /// containers (MPEG-PS, for one) only reveal their tracks after playback starts.
    pub fn video_geometry(&self) -> Option<VideoGeometry> {
        let api = &self.vlc.api;
        let mut tracks: *mut *mut MediaTrackC = std::ptr::null_mut();
        // SAFETY: the media is live; `tracks` receives an array libVLC owns until released.
        let count = unsafe { (api.libvlc_media_tracks_get)(self.media, &raw mut tracks) };
        if tracks.is_null() {
            return None;
        }
        // SAFETY: libVLC returned `count` live track pointers, released right after reading.
        unsafe {
            let geometry = VideoGeometry::find(std::slice::from_raw_parts(tracks, count as usize));
            (api.libvlc_media_tracks_release)(tracks, count);
            geometry
        }
    }

    /// Starts playback.
    pub fn play(&self) -> Result<()> {
        // SAFETY: the player is live.
        if unsafe { (self.vlc.api.libvlc_media_player_play)(self.handle) } != 0 {
            bail!("libvlc_media_player_play: {}", self.vlc.last_error());
        }
        Ok(())
    }

    /// The playback position in milliseconds, or -1 before playback starts.
    pub fn time_ms(&self) -> i64 {
        // SAFETY: the player is live.
        unsafe { (self.vlc.api.libvlc_media_player_get_time)(self.handle) }
    }

    /// libVLC's decode and display counters for the media.
    pub fn stats(&self) -> MediaStats {
        let mut stats = MediaStats::default();
        // SAFETY: the media is live and `stats` matches `libvlc_media_stats_t`.
        unsafe { (self.vlc.api.libvlc_media_get_stats)(self.media, &raw mut stats) };
        stats
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let api = &self.vlc.api;
        // SAFETY: each handle is live or NULL, and is released once. `stop` joins libVLC's
        // threads, so no callback runs after it returns.
        unsafe {
            if !self.handle.is_null() {
                (api.libvlc_media_player_stop)(self.handle);
                (api.libvlc_media_player_release)(self.handle);
            }
            if !self.media.is_null() {
                (api.libvlc_media_release)(self.media);
            }
            (api.libvlc_release)(self.instance);
        }
        drop(self.sink.take());
    }
}

fn new_instance(vlc: &LibVlc, arguments: &[String]) -> Result<*mut Instance> {
    let owned = arguments
        .iter()
        .map(|argument| CString::new(argument.as_str()))
        .collect::<Result<Vec<_>, _>>()
        .context("NUL in a libVLC argument")?;
    let pointers: Vec<*const c_char> = owned.iter().map(|argument| argument.as_ptr()).collect();
    // SAFETY: `pointers` holds `len` NUL-terminated strings that outlive the call.
    let instance = unsafe { (vlc.api.libvlc_new)(pointers.len() as i32, pointers.as_ptr()) };
    if instance.is_null() {
        bail!("libvlc_new: {}", vlc.last_error());
    }
    Ok(instance)
}
