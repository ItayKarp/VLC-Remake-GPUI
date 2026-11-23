//! A [`VideoSink`] in front of the presentation path's that tone-maps HDR pictures (S7.5).
//!
//! While a [`ToneMap`] is set when the video output opens, it asks the backend for the picture
//! as planar 10-bit 4:2:0 (`I0AL`) into buffers of its own, at the size the path's sink chose,
//! and maps each into the buffer that sink gives it. Otherwise it only passes the calls on.
//! Changing the peak of a running map applies at once; turning mapping on or off takes a new
//! video output, since the format changes.

use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::ToneMap;
use super::convert::{Source, Target, convert};
use super::tone_map::Mapper;
use crate::engine::video::{Format, MAX_PLANES, VideoSink};

/// libVLC's planar 10-bit little-endian 4:2:0.
const I0AL: [u8; 4] = *b"I0AL";
/// Rows of the staging planes are padded to this many bytes.
const ROW_ALIGN: usize = 64;
/// Bytes in a 10-bit sample.
const SAMPLE: usize = 2;

/// Tone-maps pictures on their way to `inner` while a map is set.
pub struct ToneMappingSink {
    inner: Arc<dyn VideoSink>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The map the next video output uses.
    wanted: Option<ToneMap>,
    /// Whether a video output is open.
    open: bool,
    /// The running map, while the open output maps.
    active: Option<Active>,
}

/// A video output that maps: the 10-bit picture libVLC writes, and where it goes.
struct Active {
    mapper: Mapper,
    planes: [Vec<u16>; 3],
    /// Samples per row of the luma plane and of each chroma plane.
    pitches: [usize; 2],
    width: usize,
    height: usize,
    /// The inner sink's format, and the buffers it gave for the picture being written.
    target: Format,
    target_planes: [*mut c_void; MAX_PLANES],
}

// SAFETY: the target planes are only touched under the mutex, between the inner sink's `lock`
// and `unlock` on the backend's video thread, as the inner sink allows.
unsafe impl Send for State {}

impl ToneMappingSink {
    /// A sink passing pictures on to `inner`.
    pub fn new(inner: Arc<dyn VideoSink>) -> Self {
        Self { inner, state: Mutex::default() }
    }

    /// Maps pictures as `map` says from the next video output, or none with `None`; a running
    /// map takes a new peak at once. True when the picture showing now needs a new output to
    /// follow: mapping turns on or off while one is open.
    pub fn set_tone_map(&self, map: Option<ToneMap>) -> bool {
        let mut state = self.state();
        state.wanted = map;
        if let (Some(active), Some(map)) = (&mut state.active, map)
            && active.mapper.tone_map() != map
        {
            active.mapper = Mapper::new(map);
        }
        state.open && state.active.is_some() != map.is_some()
    }

    /// The map the next video output uses; `None` while it shows pictures as decoded.
    pub fn wanted(&self) -> Option<ToneMap> {
        self.state().wanted
    }

    /// The map the picture showing now goes through; `None` while none is open or it isn't
    /// mapped.
    pub fn tone_map(&self) -> Option<ToneMap> {
        self.state().active.as_ref().map(|active| active.mapper.tone_map())
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl VideoSink for ToneMappingSink {
    fn setup(&self, format: &mut Format) -> bool {
        let mut target = *format;
        if !self.inner.setup(&mut target) {
            return false;
        }
        let mut state = self.state();
        state.open = true;
        state.active = state.wanted.and_then(|map| Active::new(map, target));
        match &state.active {
            Some(active) => active.describe(format),
            None => *format = target,
        }
        true
    }

    fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
        let mut state = self.state();
        let Some(active) = &mut state.active else {
            drop(state);
            return self.inner.lock(planes);
        };
        active.target_planes = [std::ptr::null_mut(); MAX_PLANES];
        self.inner.lock(&mut active.target_planes);
        for (plane, staging) in planes.iter_mut().zip(&mut active.planes) {
            *plane = staging.as_mut_ptr().cast();
        }
    }

    fn unlock(&self) {
        if let Some(active) = &self.state().active {
            active.map();
        }
        self.inner.unlock();
    }

    fn display(&self) {
        self.inner.display();
    }

    fn cleanup(&self) {
        let mut state = self.state();
        state.open = false;
        state.active = None;
        drop(state);
        self.inner.cleanup();
    }
}

impl Active {
    /// Mapping with `map` into pictures of the inner sink's `target` format; `None` (logged) if
    /// that is a format it can't write.
    fn new(map: ToneMap, target: Format) -> Option<Self> {
        if !can_write(target) {
            return None;
        }
        let (width, height) = (target.width as usize, target.height as usize);
        let pitches = [aligned(width), aligned(width.div_ceil(2))];
        let chroma_len = pitches[1] * height.div_ceil(2);
        let planes = [vec![0; pitches[0] * height], vec![512; chroma_len], vec![512; chroma_len]];
        tracing::info!("hdr: tone-mapping {map:?} into {width}x{height}");
        Some(Self {
            mapper: Mapper::new(map),
            planes,
            pitches,
            width,
            height,
            target,
            target_planes: [std::ptr::null_mut(); MAX_PLANES],
        })
    }

    /// Tells the backend the staging planes' format.
    fn describe(&self, format: &mut Format) {
        let rows = [self.height, self.height.div_ceil(2), self.height.div_ceil(2)];
        format.chroma = I0AL;
        format.width = self.target.width;
        format.height = self.target.height;
        format.pitches = [0; MAX_PLANES];
        format.lines = [0; MAX_PLANES];
        for plane in 0..3 {
            format.pitches[plane] = (self.pitches[plane.min(1)] * SAMPLE) as u32;
            format.lines[plane] = rows[plane] as u32;
        }
    }

    /// Maps the picture just written into the inner sink's buffers, if it gave any.
    fn map(&self) {
        if self.target_planes[0].is_null() {
            return;
        }
        let source = Source {
            luma: &self.planes[0],
            cb: &self.planes[1],
            cr: &self.planes[2],
            luma_pitch: self.pitches[0],
            chroma_pitch: self.pitches[1],
            width: self.width,
            height: self.height,
        };
        // SAFETY: the inner sink's `lock` gave these planes for a picture of its format, each
        // `pitches[plane]` bytes by `lines[plane]` rows, writable until its `unlock`.
        let target = unsafe { self.target() };
        if let Some(target) = target {
            convert(&self.mapper, &source, target);
        }
    }

    /// The inner sink's planes as slices.
    ///
    /// # Safety
    ///
    /// The planes are the inner sink's buffers for a picture of `self.target`, still locked.
    unsafe fn target(&self) -> Option<Target<'_>> {
        let plane = |index: usize| {
            let len = self.target.pitches[index] as usize * self.target.lines[index] as usize;
            let start = self.target_planes[index].cast::<u8>();
            // SAFETY: the caller's promise.
            (!start.is_null()).then(|| unsafe { std::slice::from_raw_parts_mut(start, len) })
        };
        if self.target.chroma == *b"BGRA" {
            let pitch = self.target.pitches[0] as usize;
            return plane(0).map(|pixels| Target::Bgra { pixels, pitch });
        }
        Some(Target::Nv12 {
            luma: plane(0)?,
            luma_pitch: self.target.pitches[0] as usize,
            chroma: plane(1)?,
            chroma_pitch: self.target.pitches[1] as usize,
        })
    }
}

/// Whether pictures can be mapped into `target`'s format: NV12 or BGRA. Logs when not.
fn can_write(target: Format) -> bool {
    let writable = target.chroma == *b"NV12" || target.chroma == *b"BGRA";
    if !writable {
        let chroma = String::from_utf8_lossy(&target.chroma);
        tracing::warn!("hdr: can't tone-map into {chroma}; showing it as decoded");
    }
    writable
}

/// `samples` 10-bit samples padded to [`ROW_ALIGN`] bytes, in samples.
fn aligned(samples: usize) -> usize {
    (samples * SAMPLE).next_multiple_of(ROW_ALIGN) / SAMPLE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::hdr::Transfer;

    /// A BGRA sink that keeps one picture.
    #[derive(Default)]
    struct Bgra {
        pixels: Mutex<Vec<u8>>,
    }

    impl VideoSink for Bgra {
        fn setup(&self, format: &mut Format) -> bool {
            format.chroma = *b"BGRA";
            format.pitches[0] = format.width * 4;
            format.lines[0] = format.height;
            *self.pixels.lock().unwrap() = vec![0; (format.width * format.height * 4) as usize];
            true
        }
        fn lock(&self, planes: &mut [*mut c_void; MAX_PLANES]) {
            planes[0] = self.pixels.lock().unwrap().as_mut_ptr().cast();
        }
        fn unlock(&self) {}
        fn display(&self) {}
        fn cleanup(&self) {}
    }

    const PQ: ToneMap = ToneMap { transfer: Transfer::Pq, peak: 1000. };

    fn proposal() -> Format {
        Format { chroma: I0AL, width: 6, height: 4, ..Format::default() }
    }

    #[test]
    fn without_a_map_it_only_passes_the_format_on() {
        let sink = ToneMappingSink::new(Arc::new(Bgra::default()));
        let mut format = proposal();
        assert!(sink.setup(&mut format));
        assert_eq!(format.chroma, *b"BGRA");
        assert_eq!(sink.tone_map(), None);
    }

    #[test]
    fn with_a_map_it_asks_for_10_bits_and_maps_into_the_inner_sink() {
        let inner = Arc::new(Bgra::default());
        let sink = ToneMappingSink::new(inner.clone());
        assert!(!sink.set_tone_map(Some(PQ)), "no output is open yet");
        let mut format = proposal();
        assert!(sink.setup(&mut format));
        assert_eq!((format.chroma, format.pitches[0], format.lines[2]), (I0AL, 64, 2));
        let mut planes = [std::ptr::null_mut(); MAX_PLANES];
        sink.lock(&mut planes);
        // SAFETY: the staging luma plane holds 32 samples a row, 4 rows.
        let luma = unsafe { std::slice::from_raw_parts_mut(planes[0].cast::<u16>(), 32 * 4) };
        luma.fill(940);
        sink.unlock();
        assert!(inner.pixels.lock().unwrap().iter().all(|byte| *byte == 255));
        assert_eq!(sink.tone_map(), Some(PQ));
    }

    #[test]
    fn turning_mapping_on_or_off_needs_a_new_output_but_a_new_peak_doesnt() {
        let sink = ToneMappingSink::new(Arc::new(Bgra::default()));
        sink.setup(&mut proposal());
        assert!(sink.set_tone_map(Some(PQ)));
        sink.setup(&mut proposal());
        let brighter = ToneMap { peak: 4000., ..PQ };
        assert!(!sink.set_tone_map(Some(brighter)));
        assert_eq!(sink.tone_map(), Some(brighter));
        assert!(sink.set_tone_map(None));
        sink.cleanup();
        assert!(!sink.set_tone_map(Some(PQ)));
    }
}
