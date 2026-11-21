//! The bridge from libVLC 3's `vmem` video output to a [`VideoSink`] (S1.4.1, ADR 0001).
//!
//! libVLC calls these functions on its video-output thread, one picture at a time: `format`
//! when the output opens, then `lock`, a copy of the decoded picture into our planes, `unlock`
//! and `display` per picture, and `cleanup` when the output closes. They only talk to the sink
//! and never call libVLC.

use std::ffi::{c_char, c_uint, c_void};
use std::slice;
use std::sync::{Arc, Mutex, PoisonError};

use super::api::{LibVlc, MediaPlayer};
use crate::engine::media_info::VideoSize;
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

/// What libVLC's opaque pointer points at: the sink, and what the worker learned about the
/// media before playing it.
pub struct Vmem {
    sink: Arc<dyn VideoSink>,
    /// The open media's display size, read before playback; `None` when unknown.
    display: Mutex<Option<VideoSize>>,
}

impl Vmem {
    /// A bridge to `sink`.
    pub fn new(sink: Arc<dyn VideoSink>) -> Self {
        Self { sink, display: Mutex::new(None) }
    }

    /// Records the display size of the media about to play, for the next `format` call.
    pub fn set_display(&self, size: Option<VideoSize>) {
        *self.display.lock().unwrap_or_else(PoisonError::into_inner) = size;
    }

    fn display(&self) -> Option<VideoSize> {
        *self.display.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Routes `player`'s video into `vmem`, which must outlive the player (the worker releases the
/// player before dropping it).
pub fn install(vlc: &LibVlc, player: *mut MediaPlayer, vmem: &Arc<Vmem>) {
    let opaque = Arc::as_ptr(vmem).cast_mut().cast();
    // SAFETY: the player is live, and `opaque` stays valid for as long as it is (see above).
    unsafe {
        (vlc.api.libvlc_video_set_callbacks)(
            player,
            on_lock,
            Some(on_unlock),
            Some(on_display),
            opaque,
        );
        (vlc.api.libvlc_video_set_format_callbacks)(player, on_format, Some(on_cleanup));
    }
}

/// # Safety
///
/// `opaque` is the pointer [`install`] registered, and the [`Vmem`] it points to is alive.
unsafe fn vmem<'a>(opaque: *mut c_void) -> &'a Vmem {
    // SAFETY: the caller's promise.
    unsafe { &*opaque.cast::<Vmem>() }
}

unsafe extern "C" fn on_format(
    opaque: *mut *mut c_void,
    chroma: *mut c_char,
    width: *mut c_uint,
    height: *mut c_uint,
    pitches: *mut c_uint,
    lines: *mut c_uint,
) -> c_uint {
    // SAFETY: libVLC passes live pointers: our opaque, a 4-byte chroma, the size, and pitch and
    // line tables of `MAX_PLANES` entries each.
    unsafe {
        let vmem = vmem(*opaque);
        let mut format = Format {
            chroma: *chroma.cast::<[u8; 4]>(),
            width: *width,
            height: *height,
            display: vmem.display(),
            ..Format::default()
        };
        let proposed = (format.width, format.height);
        if !vmem.sink.setup(&mut format) {
            tracing::warn!("vmem: the sink refused {}x{}", proposed.0, proposed.1);
            return 0;
        }
        tracing::info!(
            "vmem: libVLC proposed {}x{}, showing {} {}x{}",
            proposed.0,
            proposed.1,
            String::from_utf8_lossy(&format.chroma),
            format.width,
            format.height,
        );
        *chroma.cast::<[u8; 4]>() = format.chroma;
        *width = format.width;
        *height = format.height;
        slice::from_raw_parts_mut(pitches, MAX_PLANES).copy_from_slice(&format.pitches);
        slice::from_raw_parts_mut(lines, MAX_PLANES).copy_from_slice(&format.lines);
    }
    // libVLC's "number of picture buffers": one, since we copy each picture out.
    1
}

unsafe extern "C" fn on_cleanup(opaque: *mut c_void) {
    // SAFETY: `opaque` is the pointer `install` registered.
    unsafe { vmem(opaque).sink.cleanup() };
}

unsafe extern "C" fn on_lock(opaque: *mut c_void, planes: *mut *mut c_void) -> *mut c_void {
    // SAFETY: `planes` is libVLC's table of `MAX_PLANES` pointers; `opaque` is ours.
    unsafe { vmem(opaque).sink.lock(&mut *planes.cast::<[*mut c_void; MAX_PLANES]>()) };
    std::ptr::null_mut()
}

unsafe extern "C" fn on_unlock(opaque: *mut c_void, _picture: *mut c_void, _: *const *mut c_void) {
    // SAFETY: `opaque` is the pointer `install` registered.
    unsafe { vmem(opaque).sink.unlock() };
}

unsafe extern "C" fn on_display(opaque: *mut c_void, _picture: *mut c_void) {
    // SAFETY: `opaque` is the pointer `install` registered.
    unsafe { vmem(opaque).sink.display() };
}
