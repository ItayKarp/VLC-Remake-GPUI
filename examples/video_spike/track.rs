//! The video track's geometry, read from libVLC before playback starts.
//!
//! libVLC 3's `vmem` output proposes the decoder's *coded* size (1920×1088 for most 1080p
//! H.264) and storage pixels (720×480 for 16:9 DVD). Answering it with the visible size in
//! square pixels makes libVLC crop and scale for us, so the window never shows padding rows
//! or squashed pictures.

use std::ffi::{c_char, c_int, c_uint};

/// `libvlc_track_video`.
const TRACK_VIDEO: c_int = 1;
/// `libvlc_video_orient_left_top` and the three after it swap width and height.
const FIRST_TRANSPOSED_ORIENTATION: c_int = 4;
/// Pixel aspect ratios this close to 1:1 are treated as square: rescaling a picture by a
/// pixel or two costs a software scaler pass and shows no difference.
const SQUARE_TOLERANCE: f64 = 0.01;

/// `libvlc_video_track_t` from libVLC 3, field for field.
#[repr(C)]
pub struct VideoTrackC {
    height: c_uint,
    width: c_uint,
    sar_num: c_uint,
    sar_den: c_uint,
    frame_rate_num: c_uint,
    frame_rate_den: c_uint,
    orientation: c_int,
    projection: c_int,
    pose: [f32; 4],
}

/// `libvlc_media_track_t` from libVLC 3, field for field.
#[repr(C)]
pub struct MediaTrackC {
    codec: u32,
    original_fourcc: u32,
    id: c_int,
    kind: c_int,
    profile: c_int,
    level: c_int,
    /// Points to a `libvlc_video_track_t` when `kind` is video.
    detail: *mut VideoTrackC,
    bitrate: c_uint,
    language: *mut c_char,
    description: *mut c_char,
}

/// The parts of a video track that decide its on-screen size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoGeometry {
    /// Visible width in storage pixels.
    pub width: u32,
    /// Visible height in storage pixels.
    pub height: u32,
    /// Sample (pixel) aspect ratio; `0` in either part means unknown, read as square.
    pub sar: (u32, u32),
    /// Whether metadata rotates the picture by 90° or 270° (or transposes it).
    pub transposed: bool,
}

impl VideoGeometry {
    /// The first video track among `tracks` whose size is known. MPEG-TS parsing reports
    /// video tracks as 0×0 until the decoder starts; those count as unknown.
    ///
    /// # Safety
    ///
    /// Every pointer in `tracks` is a live `libvlc_media_track_t`.
    pub unsafe fn find(tracks: &[*mut MediaTrackC]) -> Option<Self> {
        tracks.iter().find_map(|&track| {
            // SAFETY: the caller promises live tracks; a video track's detail is a video track.
            let track = unsafe { &*track };
            let video = (track.kind == TRACK_VIDEO && !track.detail.is_null())
                .then(|| unsafe { &*track.detail })?;
            Self {
                width: video.width,
                height: video.height,
                sar: (video.sar_num, video.sar_den),
                transposed: video.orientation >= FIRST_TRANSPOSED_ORIENTATION,
            }
            .known()
        })
    }

    /// `self`, unless its size is still unknown (zero).
    fn known(self) -> Option<Self> {
        (self.width > 0 && self.height > 0).then_some(self)
    }

    /// The size to show the picture at, in square pixels and after rotation.
    pub fn display_size(self) -> (u32, u32) {
        let (num, den) = self.sar;
        let square = num == 0
            || den == 0
            || (f64::from(num) / f64::from(den) - 1.0).abs() < SQUARE_TOLERANCE;
        let width = if square {
            self.width
        } else {
            ((u64::from(self.width) * u64::from(num) + u64::from(den) / 2) / u64::from(den)) as u32
        };
        if self.transposed { (self.height, width) } else { (width, self.height) }
    }
}

#[cfg(test)]
mod tests {
    use super::VideoGeometry;

    fn geometry(width: u32, height: u32, sar: (u32, u32), transposed: bool) -> VideoGeometry {
        VideoGeometry { width, height, sar, transposed }
    }

    #[test]
    fn square_pixels_keep_their_size() {
        assert_eq!(geometry(1919, 1079, (1, 1), false).display_size(), (1919, 1079));
        assert_eq!(geometry(1920, 1080, (0, 0), false).display_size(), (1920, 1080));
        assert_eq!(geometry(1919, 1079, (17264, 17271), false).display_size(), (1919, 1079));
    }

    #[test]
    fn zero_sizes_are_unknown() {
        assert!(geometry(0, 0, (0, 0), false).known().is_none());
        assert!(geometry(1920, 1080, (1, 1), false).known().is_some());
    }

    #[test]
    fn anamorphic_dvd_widens_to_sixteen_by_nine() {
        assert_eq!(geometry(720, 480, (32, 27), false).display_size(), (853, 480));
    }

    #[test]
    fn rotated_video_swaps_its_sides() {
        assert_eq!(geometry(1920, 1080, (1, 1), true).display_size(), (1080, 1920));
    }
}
