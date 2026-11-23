//! Decoding a file's audio for its waveform profile (S7.8.2): a libVLC instance and player of
//! their own stream the audio through `transcode`, as 16-bit mono at [`RATE`], into `smem`,
//! which hands each block to [`on_samples`]. With `time-sync` off, `smem` doesn't wait for the
//! clock, so the file decodes as fast as the decoder goes rather than as it plays (a 90 s MP3
//! in about 0.3 s). Only the `no-time-sync` spelling turns it off: `time-sync=0` plays in real
//! time. Video and
//! subtitles aren't decoded.

use std::ffi::{CString, c_uint, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError, mpsc};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};

use super::api::{Event, Instance, LibVlc, MediaPlayer};
use super::events::{ENCOUNTERED_ERROR, END_REACHED};
use crate::engine::waveform::{PROGRESS_EVERY, Profile};

/// The instance's arguments: no video, subtitles, on-screen text or sound of its own.
const ARGUMENTS: [&str; 6] = [
    "--no-video-title-show",
    "--no-spu",
    "--no-osd",
    "--no-sub-autodetect-file",
    "--vout=vdummy",
    "--aout=adummy",
];
/// The rate audio is decoded at: plenty for a profile's loudness, and little to go through.
const RATE: u32 = 8000;
/// The sample size `transcode` is asked for.
const BITS: c_uint = 16;
/// The media's options besides the stream output: no video or subtitle streams.
const MEDIA_OPTIONS: [&str; 2] = [":no-sout-video", ":no-sout-spu"];

/// What `smem` writes into and reports to, through its `audio-data` pointer.
#[derive(Debug)]
struct Collector {
    /// The buffer `smem` copies the next block into.
    buffer: Mutex<Vec<u8>>,
    profile: Mutex<Profile>,
}

/// Decodes the audio of the file at `path`, `length` long, into a [`Profile`], calling
/// `progress` with the profile so far every [`PROGRESS_EVERY`]. Stops early when `cancelled`.
pub fn decode(
    path: &Path,
    length: Duration,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(&Profile),
) -> Result<Profile> {
    let vlc = super::library()?;
    let instance = super::new_instance(&vlc, &ARGUMENTS)?;
    let collector =
        Collector { buffer: Mutex::default(), profile: Mutex::new(Profile::new(length)) };
    let result = Run::new(&vlc, instance).and_then(|(run, ended)| {
        run.play(path, &collector)?;
        while !cancelled.load(Ordering::Relaxed) {
            match ended.recv_timeout(PROGRESS_EVERY) {
                Ok(true) => return Ok(()),
                Ok(false) => bail!("libVLC couldn't decode it"),
                Err(mpsc::RecvTimeoutError::Timeout) => progress(&lock(&collector.profile)),
                Err(mpsc::RecvTimeoutError::Disconnected) => bail!("the player went away"),
            }
        }
        bail!("cancelled")
    });
    // SAFETY: the instance is live; its player is released (`Run` dropped), so nothing uses it.
    unsafe { (vlc.api.libvlc_release)(instance) };
    result?;
    let profile = lock(&collector.profile).clone();
    Ok(profile)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A player decoding one file, stopped and released when dropped.
struct Run<'a> {
    vlc: &'a LibVlc,
    instance: *mut Instance,
    player: *mut MediaPlayer,
    /// Where the event callback says how decoding ended, boxed so its address stays put.
    ended: Box<mpsc::Sender<bool>>,
}

impl<'a> Run<'a> {
    /// A player on `instance`, and where it says decoding ended (`true`) or failed.
    fn new(vlc: &'a LibVlc, instance: *mut Instance) -> Result<(Self, mpsc::Receiver<bool>)> {
        // SAFETY: the instance is live.
        let player = unsafe { (vlc.api.libvlc_media_player_new)(instance) };
        if player.is_null() {
            bail!("libvlc_media_player_new: {}", vlc.last_error());
        }
        let (sender, receiver) = mpsc::channel();
        let run = Self { vlc, instance, player, ended: Box::new(sender) };
        let data = std::ptr::from_ref::<mpsc::Sender<bool>>(&run.ended).cast_mut().cast();
        // SAFETY: the player is live; `data` outlives it, released in `drop` after the player.
        unsafe {
            let events = (vlc.api.libvlc_media_player_event_manager)(player);
            for kind in [END_REACHED, ENCOUNTERED_ERROR] {
                (vlc.api.libvlc_event_attach)(events, kind, on_event, data);
            }
        }
        Ok((run, receiver))
    }

    /// Starts streaming the file at `path` into `collector`, which outlives the player.
    fn play(&self, path: &Path, collector: &Collector) -> Result<()> {
        let api = &self.vlc.api;
        let path = CString::new(path.to_str().context("the path isn't UTF-8")?)?;
        let options = [stream_output(collector)]
            .into_iter()
            .chain(MEDIA_OPTIONS.map(str::to_owned))
            .map(CString::new)
            .collect::<Result<Vec<_>, _>>()?;
        // SAFETY: the instance and player are live; the strings outlive the calls, and the
        // media's reference goes to the player before ours is released.
        unsafe {
            let media = (api.libvlc_media_new_path)(self.instance, path.as_ptr());
            if media.is_null() {
                bail!("libvlc_media_new_path: {}", self.vlc.last_error());
            }
            for option in &options {
                (api.libvlc_media_add_option)(media, option.as_ptr());
            }
            (api.libvlc_media_player_set_media)(self.player, media);
            (api.libvlc_media_release)(media);
            if (api.libvlc_media_player_play)(self.player) != 0 {
                bail!("libvlc_media_player_play: {}", self.vlc.last_error());
            }
        }
        Ok(())
    }
}

impl Drop for Run<'_> {
    fn drop(&mut self) {
        let api = &self.vlc.api;
        // SAFETY: the player is live and released once; `stop` joins libVLC's threads, so no
        // callback runs after it and the sender and collector may then go.
        unsafe {
            (api.libvlc_media_player_stop)(self.player);
            (api.libvlc_media_player_release)(self.player);
        }
    }
}

/// The `sout` option streaming the audio into `smem`, which calls back with `collector`.
fn stream_output(collector: &Collector) -> String {
    let data = std::ptr::from_ref(collector) as usize;
    let before = on_buffer as *const () as usize;
    let after = on_samples as *const () as usize;
    format!(
        ":sout=#transcode{{acodec=s16l,channels=1,samplerate={RATE}}}:smem{{\
         audio-prerender-callback={before},audio-postrender-callback={after},\
         audio-data={data},no-time-sync}}"
    )
}

/// `smem` asks for a buffer of `size` bytes to copy the next block into.
///
/// # Safety
///
/// `data` is the [`Collector`] [`stream_output`] named, alive until the player stops, and
/// `buffer` is writable.
unsafe extern "C" fn on_buffer(data: *mut c_void, buffer: *mut *mut u8, size: usize) {
    // SAFETY: the caller's promise.
    let collector = unsafe { &*data.cast::<Collector>() };
    let mut held = lock(&collector.buffer);
    held.resize(size, 0);
    // SAFETY: the caller's promise; the buffer stays put until the next call resizes it, after
    // `on_samples` read it.
    unsafe { *buffer = held.as_mut_ptr() };
}

/// `smem` copied a block of `samples` frames into the buffer.
///
/// # Safety
///
/// `data` is the [`Collector`] [`stream_output`] named, alive until the player stops.
#[allow(clippy::too_many_arguments, reason = "smem's callback signature")]
unsafe extern "C" fn on_samples(
    data: *mut c_void,
    _buffer: *mut u8,
    channels: c_uint,
    rate: c_uint,
    samples: c_uint,
    bits: c_uint,
    size: usize,
    _pts: i64,
) {
    // SAFETY: the caller's promise.
    let collector = unsafe { &*data.cast::<Collector>() };
    if bits != BITS {
        return;
    }
    let held = lock(&collector.buffer);
    let channels = channels as usize;
    let bytes = (samples as usize * channels * 2).min(size).min(held.len());
    let pcm: Vec<i16> =
        held[..bytes].as_chunks::<2>().0.iter().map(|pair| i16::from_le_bytes(*pair)).collect();
    drop(held);
    lock(&collector.profile).push(&pcm, channels, rate);
}

/// libVLC's event callback: says whether decoding reached the end or failed. Never calls
/// libVLC.
unsafe extern "C" fn on_event(event: *const Event, data: *mut c_void) {
    // SAFETY: libVLC passes a live event, and `data` is the boxed sender the run owns until
    // after it releases the player.
    let (kind, ended) = unsafe { ((*event).kind, &*data.cast::<mpsc::Sender<bool>>()) };
    // An error means the run is over and no longer listens.
    let _ = ended.send(kind == END_REACHED);
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    use crate::engine::audio::test_tone;
    use crate::engine::waveform::profile::BUCKETS;

    #[test]
    #[ignore = "needs libVLC"]
    fn the_test_tone_is_loud_where_it_sounds_and_silent_between() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { super::super::runtime::prepare_environment() };
        let tone = super::super::test_tone::tone_file().expect("the tone");
        let mut reports = 0;
        let started = std::time::Instant::now();
        let profile = super::decode(&tone, test_tone::LENGTH, &AtomicBool::new(false), |_| {
            reports += 1;
        })
        .expect("decoded");
        // Faster than it plays: `smem` doesn't wait for the clock.
        assert!(started.elapsed() < test_tone::LENGTH / 2, "took {:?}", started.elapsed());
        let peaks = profile.peaks(true);
        let at = |time: Duration| {
            let fraction = time.as_secs_f32() / test_tone::LENGTH.as_secs_f32();
            peaks.level_between(fraction, fraction).expect("complete")
        };
        assert!(at(Duration::from_millis(400)) > 0.5, "the left tone");
        assert!(at(Duration::from_millis(925)) < 0.1, "the silence");
        assert!(at(Duration::from_millis(1450)) > 0.5, "the right tone");
        assert_eq!(peaks.levels().len(), BUCKETS);
    }
}
