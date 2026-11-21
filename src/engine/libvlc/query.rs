//! Reads the media's description from libVLC (tracks, video size, chapters, metadata) on the
//! worker thread, and sends it to the player.

use std::ffi::c_int;
use std::time::Duration;

use super::api::{MediaTrack, TrackDescription, VideoTrack, borrowed_string};
use super::worker::{Refresh, Worker};
use crate::engine::backend::BackendEvent;
use crate::engine::media_info::{Chapter, Metadata, Track, TrackId, TrackKind, Tracks, VideoSize};

/// `libvlc_meta_Title`, `libvlc_meta_Artist` and `libvlc_meta_Album`.
const META_TITLE: c_int = 0;
const META_ARTIST: c_int = 1;
const META_ALBUM: c_int = 4;
/// `libvlc_video_orient_left_top` and the three after it swap width and height.
const FIRST_TRANSPOSED_ORIENTATION: c_int = 4;
/// The chapters of the current title, for `libvlc_media_player_get_full_chapter_descriptions`.
const CURRENT_TITLE: c_int = -1;

/// One elementary stream as `libvlc_media_tracks_get` describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stream {
    id: TrackId,
    kind: Option<TrackKind>,
    language: Option<String>,
    size: Option<VideoSize>,
}

impl Worker {
    /// Reads `what` again and sends it to the player.
    pub(super) fn refresh(&self, what: Refresh) {
        match what {
            Refresh::Tracks => self.send_tracks(),
            Refresh::Chapters => self.send_chapters(),
            Refresh::Metadata => self.send_metadata(),
            Refresh::Everything => {
                self.apply_delays();
                self.send_tracks();
                self.send_chapters();
                self.send_metadata();
            }
        }
    }

    /// How many streams the media has, of any kind.
    pub(super) fn track_count(&self) -> usize {
        self.streams().len()
    }

    fn send_tracks(&self) {
        let streams = self.streams();
        let size = streams.iter().find_map(|stream| stream.size);
        let mut tracks = Tracks::default();
        for kind in [TrackKind::Audio, TrackKind::Video, TrackKind::Subtitle] {
            tracks.all.extend(merge(kind, &self.names(kind), &streams));
            tracks.select(kind, self.selected(kind));
        }
        self.shared.send(BackendEvent::Tracks(tracks));
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
                        title: borrowed_string((*chapter).name),
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
            self.vlc.take_string((self.vlc.api.libvlc_media_get_meta)(self.media, kind))
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
                .map(|&track| stream(&*track))
                .collect();
            (api.libvlc_media_tracks_release)(list, count);
            streams
        }
    }

    /// The player's names for the tracks of `kind`, as VLC shows them ("Track 1 - [English]").
    fn names(&self, kind: TrackKind) -> Vec<(TrackId, Option<String>)> {
        let api = &self.vlc.api;
        let describe = match kind {
            TrackKind::Audio => api.libvlc_audio_get_track_description,
            TrackKind::Video => api.libvlc_video_get_track_description,
            TrackKind::Subtitle => api.libvlc_video_get_spu_description,
        };
        // SAFETY: the player is live; the list is NULL or a chain we release once.
        unsafe {
            let first = describe(self.handles.player);
            let mut names = Vec::new();
            let mut node: *const TrackDescription = first;
            while let Some(entry) = node.as_ref() {
                names.push((TrackId(entry.id), borrowed_string(entry.name)));
                node = entry.next;
            }
            if !first.is_null() {
                (api.libvlc_track_description_list_release)(first);
            }
            names
        }
    }

    fn selected(&self, kind: TrackKind) -> Option<TrackId> {
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

/// Reads one `libvlc_media_track_t`.
///
/// # Safety
///
/// `track` is live, and its `detail` is NULL or a live video track when it is video.
unsafe fn stream(track: &MediaTrack) -> Stream {
    let kind = match track.kind {
        0 => Some(TrackKind::Audio),
        1 => Some(TrackKind::Video),
        2 => Some(TrackKind::Subtitle),
        _ => None,
    };
    let video = if kind == Some(TrackKind::Video) {
        // SAFETY: a video track's detail is a video track, or NULL.
        unsafe { track.detail.as_ref() }
    } else {
        None
    };
    Stream {
        id: TrackId(track.id),
        kind,
        // SAFETY: libVLC's strings are NULL or NUL-terminated.
        language: unsafe { borrowed_string(track.language) },
        size: video.and_then(display_size),
    }
}

/// The size a video track shows at: square pixels, after rotation. `None` while unknown (MPEG-TS
/// reports 0×0 until decoding starts).
fn display_size(video: &VideoTrack) -> Option<VideoSize> {
    if video.width == 0 || video.height == 0 {
        return None;
    }
    let (num, den) = (u64::from(video.sar_num), u64::from(video.sar_den));
    let width = if num == 0 || den == 0 {
        video.width
    } else {
        ((u64::from(video.width) * num + den / 2) / den) as u32
    };
    let (width, height) = if video.orientation >= FIRST_TRANSPOSED_ORIENTATION {
        (video.height, width)
    } else {
        (width, video.height)
    };
    Some(VideoSize { width, height })
}

/// The tracks of `kind`: the player's list (with names) once playing, else the media's streams.
/// Languages come from the streams; the "Disable" entry (id -1) is left out.
fn merge(kind: TrackKind, names: &[(TrackId, Option<String>)], streams: &[Stream]) -> Vec<Track> {
    let language = |id| streams.iter().find(|s| s.id == id).and_then(|s| s.language.clone());
    let listed = names.iter().filter(|(id, _)| id.0 >= 0);
    if names.iter().any(|(id, _)| id.0 >= 0) {
        return listed
            .map(|(id, name)| Track { id: *id, kind, name: name.clone(), language: language(*id) })
            .collect();
    }
    streams
        .iter()
        .filter(|stream| stream.kind == Some(kind))
        .map(|stream| Track { id: stream.id, kind, name: None, language: stream.language.clone() })
        .collect()
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
        assert_eq!(display_size(&video(1920, 1080, (0, 0), 5)), size(1080, 1920));
        assert_eq!(display_size(&video(0, 0, (1, 1), 0)), None);
    }

    #[test]
    fn player_names_win_and_streams_fill_in_before_playback() {
        let stream = |id, language: &str| Stream {
            id: TrackId(id),
            kind: Some(TrackKind::Audio),
            language: Some(language.into()),
            size: None,
        };
        let streams = [stream(1, "en"), stream(2, "fr")];
        let names = [(TrackId(-1), Some("Disable".into())), (TrackId(2), Some("Track 2".into()))];
        let from_player = merge(TrackKind::Audio, &names, &streams);
        assert_eq!(from_player.len(), 1);
        assert_eq!(
            (from_player[0].name.as_deref(), from_player[0].language.as_deref()),
            (Some("Track 2"), Some("fr"))
        );
        let unnamed = merge(TrackKind::Audio, &[], &streams);
        assert_eq!(unnamed.iter().map(|track| track.id.0).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(merge(TrackKind::Subtitle, &[], &streams), []);
    }
}
