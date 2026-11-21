//! The bridge between libVLC's `vmem` callbacks and a Rust [`VideoSink`].
//!
//! libVLC 3's `vmem` output calls, for every picture and on its one video-output thread:
//! `lock` (we hand out plane pointers), a CPU copy of the decoded picture into them, `unlock`,
//! then `display` when the picture is due. A sink therefore never sees two pictures in flight.

use std::ffi::{c_char, c_uint, c_void};
use std::slice;
use std::sync::Arc;

use crate::libvlc::{LibVlc, MediaPlayer};

/// libVLC's `PICTURE_PLANE_MAX`: the length of the plane, pitch and line tables.
pub const MAX_PLANES: usize = 5;

/// The picture format libVLC proposes, and the sink's answer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Format {
    /// A VLC fourcc such as `BGRA` or `NV12`.
    pub chroma: [u8; 4],
    /// Picture width in pixels; libVLC scales to whatever the sink answers.
    pub width: u32,
    /// Picture height in pixels.
    pub height: u32,
    /// Bytes per row of each plane.
    pub pitches: [u32; MAX_PLANES],
    /// Rows of each plane.
    pub lines: [u32; MAX_PLANES],
}

/// Receives decoded pictures from libVLC. Every method runs on libVLC's video-output thread.
pub trait VideoSink: Send + Sync {
    /// Agrees on the format, editing `format` in place; `false` refuses the video.
    fn setup(&self, format: &mut Format) -> bool;
    /// Fills `planes` with buffers for the next picture.
    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]);
    /// libVLC finished writing the locked picture.
    fn unlock(&self);
    /// The last unlocked picture is due on screen.
    fn display(&self);
}

/// The sink as libVLC's opaque pointer: a thin pointer to a boxed fat `Arc`.
type Opaque = Arc<dyn VideoSink>;

/// Routes `player`'s video into `sink`. The returned handle owns the sink's box and must be
/// dropped only after the player stopped, when libVLC calls back no more.
pub fn install(vlc: &LibVlc, player: *mut MediaPlayer, sink: Arc<dyn VideoSink>) -> SinkHandle {
    let opaque = Box::into_raw(Box::new(sink));
    // SAFETY: `player` is live, and `opaque` stays valid until the `SinkHandle` drops.
    unsafe {
        (vlc.api.libvlc_video_set_callbacks)(
            player,
            on_lock,
            Some(on_unlock),
            Some(on_display),
            opaque.cast(),
        );
        (vlc.api.libvlc_video_set_format_callbacks)(player, on_format, None);
    }
    SinkHandle(opaque)
}

/// Keeps the sink alive for libVLC's callbacks; see [`install`].
pub struct SinkHandle(*mut Opaque);

impl Drop for SinkHandle {
    fn drop(&mut self) {
        // SAFETY: the pointer came from `Box::into_raw` in `install` and is freed once.
        drop(unsafe { Box::from_raw(self.0) });
    }
}

/// # Safety
///
/// `opaque` is the pointer `install` registered.
unsafe fn sink<'a>(opaque: *mut c_void) -> &'a Opaque {
    // SAFETY: the caller promises the pointer `install` registered, alive until `SinkHandle` drops.
    unsafe { &*opaque.cast::<Opaque>() }
}

unsafe extern "C" fn on_format(
    opaque: *mut *mut c_void,
    chroma: *mut c_char,
    width: *mut c_uint,
    height: *mut c_uint,
    pitches: *mut c_uint,
    lines: *mut c_uint,
) -> c_uint {
    // SAFETY: libVLC passes live pointers: our opaque, a 4-byte chroma (plus NUL), the size,
    // and pitch and line tables of `MAX_PLANES` entries each.
    unsafe {
        let mut format = Format {
            chroma: *chroma.cast::<[u8; 4]>(),
            width: *width,
            height: *height,
            ..Format::default()
        };
        let proposed = String::from_utf8_lossy(&format.chroma).into_owned();
        let (source_width, source_height) = (format.width, format.height);
        if !sink(*opaque).setup(&mut format) {
            return 0;
        }
        eprintln!(
            "video format: libVLC proposed {proposed} {source_width}x{source_height}, got {} \
             {}x{} pitches {:?} lines {:?}",
            String::from_utf8_lossy(&format.chroma),
            format.width,
            format.height,
            &format.pitches[..2],
            &format.lines[..2],
        );
        *chroma.cast::<[u8; 4]>() = format.chroma;
        *width = format.width;
        *height = format.height;
        slice::from_raw_parts_mut(pitches, MAX_PLANES).copy_from_slice(&format.pitches);
        slice::from_raw_parts_mut(lines, MAX_PLANES).copy_from_slice(&format.lines);
    }
    1
}

unsafe extern "C" fn on_lock(opaque: *mut c_void, planes: *mut *mut c_void) -> *mut c_void {
    // SAFETY: `planes` is libVLC's table of `MAX_PLANES` pointers; `opaque` is ours.
    unsafe { sink(opaque).lock(&mut *planes.cast::<[*mut c_void; MAX_PLANES]>()) };
    std::ptr::null_mut()
}

unsafe extern "C" fn on_unlock(opaque: *mut c_void, _picture: *mut c_void, _: *const *mut c_void) {
    // SAFETY: `opaque` is the pointer `install` registered.
    unsafe { sink(opaque).unlock() };
}

unsafe extern "C" fn on_display(opaque: *mut c_void, _picture: *mut c_void) {
    // SAFETY: `opaque` is the pointer `install` registered.
    unsafe { sink(opaque).display() };
}

/// What decides the picture size a sink asks libVLC for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    /// The track's display size, when it was read before playback.
    pub display: Option<(u32, u32)>,
    /// The largest texture side to allow.
    pub max_texture: u32,
}

/// The size to ask libVLC for: the track's display size when known (so libVLC crops coded
/// padding and squares anamorphic pixels), else its proposal, shrunk to fit `max_texture`.
pub fn choose_size(proposed: &Format, display: Option<(u32, u32)>, max_texture: u32) -> (u32, u32) {
    let (width, height) = display.unwrap_or((proposed.width, proposed.height));
    fit_texture(width, height, max_texture)
}

/// Shrinks `width`×`height` to fit `max` on both sides, keeping the aspect ratio and even
/// sizes (4:2:0 chroma needs them). Sizes that already fit come back unchanged.
pub fn fit_texture(width: u32, height: u32, max: u32) -> (u32, u32) {
    if width <= max && height <= max {
        return (width, height);
    }
    let longest = u64::from(width.max(height));
    let even = |side: u32| ((u64::from(side) * u64::from(max) / longest) as u32 & !1).max(2);
    (even(width), even(height))
}

#[cfg(test)]
mod tests {
    use super::fit_texture;

    #[test]
    fn sizes_that_fit_are_unchanged() {
        assert_eq!(fit_texture(1919, 1079, 8192), (1919, 1079));
        assert_eq!(fit_texture(8192, 4320, 8192), (8192, 4320));
    }

    #[test]
    fn oversized_video_shrinks_to_the_limit_keeping_aspect() {
        assert_eq!(fit_texture(15360, 8640, 8192), (8192, 4608));
        assert_eq!(fit_texture(4320, 7680, 4096), (2304, 4096));
    }

    #[test]
    fn shrunk_sizes_are_even() {
        let (width, height) = fit_texture(16385, 9999, 8191);
        assert_eq!((width % 2, height % 2), (0, 0));
    }
}
