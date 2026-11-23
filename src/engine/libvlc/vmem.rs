//! The bridge from libVLC 3's `vmem` video output to a [`VideoSink`] (S1.4.1, ADR 0001).
//!
//! libVLC calls these functions on its video-output thread, one picture at a time: `format`
//! when the output opens, then `lock`, a copy of the decoded picture into our planes, `unlock`
//! and `display` per picture, and `cleanup` when the output closes. They only talk to the sink,
//! and to the worker once a picture it waits for shows, and never call libVLC.

use std::ffi::{c_char, c_uint, c_void};
use std::slice;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use super::api::{LibVlc, MediaPlayer};
use crate::engine::hdr::{ToneMap, ToneMappingSink};
use crate::engine::media_info::VideoSize;
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

/// What libVLC's opaque pointer points at: the sink, tone-mapping HDR when asked (S7.5), and
/// what the worker learned about the media before playing it.
pub struct Vmem {
    sink: ToneMappingSink,
    /// The open media's display size, read before playback; `None` when unknown.
    display: Mutex<Option<VideoSize>>,
    /// Whether the user turned the picture a quarter turn (S7.2): libVLC's filters give it
    /// squeezed into the unturned size, so the sink asks for the sides swapped.
    transposed: AtomicBool,
    /// What to do once the next picture shows (`hold.rs`).
    next_picture: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl Vmem {
    /// A bridge to `sink`.
    pub fn new(sink: Arc<dyn VideoSink>) -> Self {
        Self {
            sink: ToneMappingSink::new(sink),
            display: Mutex::new(None),
            transposed: AtomicBool::new(false),
            next_picture: Mutex::new(None),
        }
    }

    /// Runs `then` once the next picture shows, on libVLC's video-output thread; it must not
    /// call libVLC.
    pub fn after_next_picture(&self, then: Box<dyn FnOnce() + Send>) {
        *self.next_picture.lock().unwrap_or_else(PoisonError::into_inner) = Some(then);
    }

    /// Drops what [`Self::after_next_picture`] asked for, if it hasn't run.
    pub fn forget_next_picture(&self) {
        self.next_picture.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    /// Tone-maps pictures as `map` says from the next video output; true when the picture
    /// showing now needs a new output to follow (see [`ToneMappingSink::set_tone_map`]).
    pub fn set_tone_map(&self, map: Option<ToneMap>) -> bool {
        self.sink.set_tone_map(map)
    }

    /// How pictures are tone-mapped from the next video output; `None` while they show as
    /// decoded.
    pub fn wanted_tone_map(&self) -> Option<ToneMap> {
        self.sink.wanted()
    }

    /// How the picture showing now is tone-mapped; `None` while it shows as decoded.
    pub fn tone_map(&self) -> Option<ToneMap> {
        self.sink.tone_map()
    }

    /// Records the display size of the media about to play, for the next `format` call.
    pub fn set_display(&self, size: Option<VideoSize>) {
        *self.display.lock().unwrap_or_else(PoisonError::into_inner) = size;
    }

    /// Whether the display size of the media about to play is known: its parse found video.
    pub fn knows_display(&self) -> bool {
        self.display().is_some()
    }

    /// Records whether the picture is turned a quarter, for the next `format` call.
    pub fn set_transposed(&self, transposed: bool) {
        self.transposed.store(transposed, Ordering::Relaxed);
    }

    fn display(&self) -> Option<VideoSize> {
        *self.display.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The size libVLC `proposed` and the display size, with their sides swapped when the
    /// picture is turned a quarter.
    fn turned(&self, proposed: (c_uint, c_uint)) -> ((c_uint, c_uint), Option<VideoSize>) {
        let display = self.display();
        if !self.transposed.load(Ordering::Relaxed) {
            return (proposed, display);
        }
        let display = display.map(|size| VideoSize { width: size.height, height: size.width });
        ((proposed.1, proposed.0), display)
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
        let ((proposed_width, proposed_height), display) = vmem.turned((*width, *height));
        let mut format = Format {
            chroma: *chroma.cast::<[u8; 4]>(),
            width: proposed_width,
            height: proposed_height,
            display,
            ..Format::default()
        };
        let proposed = (format.width, format.height);
        if !vmem.sink.setup(&mut format) {
            tracing::warn!("vmem: the sink refused {}x{}", proposed.0, proposed.1);
            return 0;
        }
        log_format(proposed, &format);
        *chroma.cast::<[u8; 4]>() = format.chroma;
        *width = format.width;
        *height = format.height;
        slice::from_raw_parts_mut(pitches, MAX_PLANES).copy_from_slice(&format.pitches);
        slice::from_raw_parts_mut(lines, MAX_PLANES).copy_from_slice(&format.lines);
    }
    // libVLC's "number of picture buffers": one, since we copy each picture out.
    1
}

/// Logs the size libVLC `proposed` and the picture `format` the sink chose.
fn log_format(proposed: (c_uint, c_uint), format: &Format) {
    tracing::info!(
        "vmem: libVLC proposed {}x{}, showing {} {}x{}",
        proposed.0,
        proposed.1,
        String::from_utf8_lossy(&format.chroma),
        format.width,
        format.height,
    );
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
    let vmem = unsafe { vmem(opaque) };
    vmem.sink.display();
    let then = vmem.next_picture.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some(then) = then {
        then();
    }
}
