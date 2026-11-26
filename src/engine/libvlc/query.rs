//! Reads the media's description from libVLC (tracks, video size, chapters, metadata) on the
//! worker thread, and sends it to the player.

use std::ffi::c_int;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::api::{MediaTrack, TrackDescription, VideoTrack, borrowed_string};
use super::track_list::{self, Stream};
use super::worker::{Refresh, Worker};
use crate::engine::backend::BackendEvent;
use crate::engine::display_text::clean_tag;
use crate::engine::media_info::{Chapter, Metadata, VideoSize};
use crate::engine::tracks::{TrackId, TrackKind, Tracks};

/// `libvlc_meta_Title`, `libvlc_meta_Artist` and `libvlc_meta_Album`.
const META_TITLE: c_int = 0;
const META_ARTIST: c_int = 1;
const META_ALBUM: c_int = 4;
/// `libvlc_video_orient_left_top` and the three after it swap width and height.
pub(super) const FIRST_TRANSPOSED_ORIENTATION: c_int = 4;
/// Pixel aspect ratios this close to 1:1 count as square: rescaling a picture by a pixel or two
/// costs a scaler pass and shows no difference (`odd-1919x1079.mkv` is 17264:17271).
const SQUARE_TOLERANCE: f64 = 0.01;
/// The chapters of the current title, for `libvlc_media_player_get_full_chapter_descriptions`.
const CURRENT_TITLE: c_int = -1;

impl Worker {
    /// Reads `what` again and sends it to the player.
    pub(super) fn refresh(&mut self, what: Refresh) {
        match what {
            Refresh::Tracks => {
                self.send_tracks();
                self.send_programs();
                self.apply_visualizer();
            }
            Refresh::Chapters => {
                self.send_titles();
                self.send_chapters();
            }
            Refresh::Metadata => self.send_metadata(),
            Refresh::AudioDevices => self.poll_devices(),
            Refresh::Audible => self.restart_visualizer(),
            Refresh::Everything => {
                self.restore_tracks();
                self.apply_delays();
                self.apply_speaker_setup();
                self.send_tracks();
                self.send_programs();
                self.send_titles();
                self.send_chapters();
                self.send_metadata();
                self.watch_colour(Instant::now());
                self.apply_visualizer();
            }
        }
    }

    /// Whether the media has a video stream, as far as libVLC knows yet.
    pub(super) fn has_video_stream(&self) -> bool {
        self.streams().iter().any(|stream| stream.kind == Some(TrackKind::Video))
    }

    /// How many streams the media has, of any kind.
    pub(super) fn track_count(&self) -> usize {
        self.streams().len()
    }

    fn send_tracks(&mut self) {
        // The reopened media's tracks show once its selection is restored.
        if self.shared.switching.load(Ordering::Relaxed) {
            return;
        }
        let streams = self.streams();
        let mut tracks = Tracks::default();
        for kind in [TrackKind::Audio, TrackKind::Video, TrackKind::Subtitle] {
            tracks.all.extend(track_list::merge(kind, &self.listed(kind), &streams));
            tracks.select(kind, self.selected(kind));
        }
        if self.shared.started.load(Ordering::Relaxed) {
            self.defaults.learn(&tracks);
        }
        self.defaults.mark(&mut tracks);
        self.mark_hardware(&tracks);
        self.watch_subtitles(&tracks, Instant::now());
        self.shared.send(BackendEvent::Tracks(tracks));
        self.send_size_of(&streams);
    }

    /// Reads the video size again; see [`super::size_watch`].
    pub(super) fn send_video_size(&self) {
        self.send_size_of(&self.streams());
    }

    fn send_size_of(&self, streams: &[Stream]) {
        let size = streams.iter().find_map(|stream| stream.size).map(|size| {
            let turned = self.video_filters.transform.swaps_sides();
            if turned { VideoSize { width: size.height, height: size.width } } else { size }
        });
        let has_video = streams.iter().any(|stream| stream.kind == Some(TrackKind::Video));
        self.size_watch.observe(has_video && size.is_none(), Instant::now());
        self.shared.send(BackendEvent::VideoSize(size));
    }

    fn send_chapters(&self) {
        let api = &self.vlc.api;
        let mut list = std::ptr::null_mut();
        // SAFETY: the player is live; libVLC fills `list` with `count` entries we release.
        let count = unsafe {
            (api.libvlc_media_player_get_full_chapter_descriptions)(
                self.handles.player,
                CURRENT_TITLE,
                &raw mut list,
            )
        };
        let mut chapters = Vec::new();
        if count > 0 && !list.is_null() {
            // SAFETY: libVLC returned `count` live entries, released right after reading.
            unsafe {
                for &chapter in std::slice::from_raw_parts(list, count as usize) {
                    chapters.push(Chapter {
                        title: clean_tag(borrowed_string((*chapter).name)),
                        start: Duration::from_millis((*chapter).time_offset.max(0) as u64),
                    });
                }
                (api.libvlc_chapter_descriptions_release)(list, count as u32);
            }
        }
        self.shared.send(BackendEvent::Chapters(chapters));
    }

    fn send_metadata(&self) {
        if self.media.is_null() {
            return;
        }
        // SAFETY: the media is live; `get_meta` returns a string we own, or NULL.
        let tag = |kind| unsafe {
            clean_tag(self.vlc.take_string((self.vlc.api.libvlc_media_get_meta)(self.media, kind)))
        };
        let metadata =
            Metadata { title: tag(META_TITLE), artist: tag(META_ARTIST), album: tag(META_ALBUM) };
        self.shared.send(BackendEvent::Metadata(metadata));
    }

    fn streams(&self) -> Vec<Stream> {
        if self.media.is_null() {
            return Vec::new();
        }
        let api = &self.vlc.api;
        let mut list: *mut *mut MediaTrack = std::ptr::null_mut();
        // SAFETY: the media is live; `list` receives `count` tracks we release.
        let count = unsafe { (api.libvlc_media_tracks_get)(self.media, &raw mut list) };
        if list.is_null() {
            return Vec::new();
        }
        // SAFETY: libVLC returned `count` live tracks, released right after reading.
        unsafe {
            let streams = std::slice::from_raw_parts(list, count as usize)
                .iter()
                .map(|&track| track_list::stream(&*track))
                .collect();
            (api.libvlc_media_tracks_release)(list, count);
            streams
        }
    }

    /// The ids of the player's tracks of `kind`, with -1 for "Disable".
    fn listed(&self, kind: TrackKind) -> Vec<TrackId> {
        let api = &self.vlc.api;
        let describe = match kind {
            TrackKind::Audio => api.libvlc_audio_get_track_description,
            TrackKind::Video => api.libvlc_video_get_track_description,
            TrackKind::Subtitle => api.libvlc_video_get_spu_description,
        };
        // SAFETY: the player is live; the list is NULL or a chain we release once.
        unsafe {
            let first = describe(self.handles.player);
            let mut ids = Vec::new();
            let mut node: *const TrackDescription = first;
            while let Some(entry) = node.as_ref() {
                ids.push(TrackId(entry.id));
                node = entry.next;
            }
            if !first.is_null() {
                (api.libvlc_track_description_list_release)(first);
            }
            ids
        }
    }

    pub(super) fn selected(&self, kind: TrackKind) -> Option<TrackId> {
        let api = &self.vlc.api;
        let current = match kind {
            TrackKind::Audio => api.libvlc_audio_get_track,
            TrackKind::Video => api.libvlc_video_get_track,
            TrackKind::Subtitle => api.libvlc_video_get_spu,
        };
        // SAFETY: the player is live.
        let id = unsafe { current(self.handles.player) };
        (id >= 0).then_some(TrackId(id))
    }
}

/// The size a video track shows at: square pixels, after rotation. `None` while unknown (MPEG-TS
/// reports 0×0 until decoding starts).
pub(super) fn display_size(video: &VideoTrack) -> Option<VideoSize> {
    if video.width == 0 || video.height == 0 {
        return None;
    }
    let (num, den) = (u64::from(video.sar_num), u64::from(video.sar_den));
    let square = num == 0 || den == 0 || (num as f64 / den as f64 - 1.0).abs() < SQUARE_TOLERANCE;
    let width =
        if square { video.width } else { ((u64::from(video.width) * num + den / 2) / den) as u32 };
    let (width, height) = if video.orientation >= FIRST_TRANSPOSED_ORIENTATION {
        (video.height, width)
    } else {
        (width, video.height)
    };
    Some(VideoSize { width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(width: u32, height: u32, sar: (u32, u32), orientation: c_int) -> VideoTrack {
        VideoTrack {
            height,
            width,
            sar_num: sar.0,
            sar_den: sar.1,
            frame_rate_num: 0,
            frame_rate_den: 0,
            orientation,
            projection: 0,
            pose: [0.0; 4],
        }
    }

    #[test]
    fn video_sizes_correct_pixel_aspect_and_rotation() {
        let size = |w, h| Some(VideoSize { width: w, height: h });
        assert_eq!(display_size(&video(1920, 1080, (1, 1), 0)), size(1920, 1080));
        assert_eq!(display_size(&video(720, 480, (32, 27), 0)), size(853, 480));
        assert_eq!(display_size(&video(1919, 1079, (17264, 17271), 0)), size(1919, 1079));
        assert_eq!(display_size(&video(1920, 1080, (0, 0), 5)), size(1080, 1920));
        assert_eq!(display_size(&video(0, 0, (1, 1), 0)), None);
    }
}
