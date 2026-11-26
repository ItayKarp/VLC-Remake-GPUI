//! Converting media (S8.10): a libVLC instance and player of their own stream the media through
//! the `sout` chain the caller gives (`transcode` into a file), as fast as the encoders go: a file
//! output doesn't wait for the clock. Nothing is shown or heard.
//!
//! The player says it reached the end once the input is read; stopping it then flushes the
//! encoders and finishes the file (an MP4's index is written on close).

use std::ffi::{CString, c_void};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Result, bail};

use super::api::{Event, Instance, LibVlc, Media as VlcMedia, MediaPlayer, MediaTrack};
use super::events::{ENCOUNTERED_ERROR, END_REACHED};
use crate::app::media::Media;

/// The instance's arguments: no window, on-screen text, sound or console log of its own, and no
/// subtitle files picked up beside the source.
const ARGUMENTS: [&str; 7] = [
    "--no-video-title-show",
    "--no-osd",
    "--no-sub-autodetect-file",
    "--no-stats",
    "--quiet",
    "--vout=vdummy",
    "--aout=adummy",
];
/// How often the caller hears how far the conversion got.
pub const TICK: Duration = Duration::from_millis(250);
/// `libvlc_track_type_t`'s audio and video.
const TRACK_AUDIO: i32 = 0;
const TRACK_VIDEO: i32 = 1;

/// How a conversion ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The whole source went through; it had `streams`.
    Finished(Streams),
    /// The caller stopped it.
    Stopped,
}

/// Which kinds of stream the source had.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Streams {
    /// It had sound.
    pub audio: bool,
    /// It had pictures.
    pub video: bool,
}

/// Streams `media` through libVLC with `options` (the `:sout=…` chain and the rest), calling
/// `tick` every [`TICK`] with how far it got (0 to 1); `tick` returns `false` to stop. Blocks
/// until it ends: run it on a thread of its own.
pub fn transcode(
    media: &Media,
    options: &[String],
    mut tick: impl FnMut(f32) -> bool,
) -> Result<Ending> {
    let vlc = super::library()?;
    let instance = super::new_instance(&vlc, &ARGUMENTS)?;
    let (mut run, ended) = Run::new(&vlc, instance)?;
    run.play(media, options)?;
    loop {
        match ended.recv_timeout(TICK) {
            Ok(true) => return Ok(Ending::Finished(run.streams())),
            Ok(false) => bail!("{}", run.error()),
            Err(mpsc::RecvTimeoutError::Timeout) if tick(run.progress()) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => return Ok(Ending::Stopped),
            Err(mpsc::RecvTimeoutError::Disconnected) => bail!("the player went away"),
        }
    }
}

/// An instance converting one media, its player and media; stopped and released when dropped.
struct Run<'a> {
    vlc: &'a LibVlc,
    instance: *mut Instance,
    player: *mut MediaPlayer,
    media: *mut VlcMedia,
    /// Where the event callback says how it ended, boxed so its address stays put.
    ended: Box<mpsc::Sender<bool>>,
}

impl<'a> Run<'a> {
    /// A player on `instance`, which the run now owns, and where it says it ended (`true`) or
    /// failed.
    fn new(vlc: &'a LibVlc, instance: *mut Instance) -> Result<(Self, mpsc::Receiver<bool>)> {
        // SAFETY: the instance is live.
        let player = unsafe { (vlc.api.libvlc_media_player_new)(instance) };
        if player.is_null() {
            // SAFETY: the instance is live and nothing else holds it.
            unsafe { (vlc.api.libvlc_release)(instance) };
            bail!("libvlc_media_player_new: {}", vlc.last_error());
        }
        let (sender, receiver) = mpsc::channel();
        let media = std::ptr::null_mut();
        let run = Self { vlc, instance, player, media, ended: Box::new(sender) };
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

    /// Starts streaming `media` with `options`.
    fn play(&mut self, media: &Media, options: &[String]) -> Result<()> {
        let (location, from_path) = match media {
            Media::File(path) => (path.to_string_lossy().into_owned(), true),
            Media::Url(url) => (url.clone(), false),
            Media::Device(device) => (device.mrl().to_owned(), false),
        };
        let own = media.as_device().map_or(&[][..], |device| device.options());
        let options = own
            .iter()
            .chain(options)
            .map(|option| CString::new(option.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        let location = CString::new(location)?;
        let api = &self.vlc.api;
        let create =
            if from_path { api.libvlc_media_new_path } else { api.libvlc_media_new_location };
        // SAFETY: the instance and player are live, and the strings outlive the calls; the
        // media is the run's, released in `drop`.
        unsafe {
            self.media = create(self.instance, location.as_ptr());
            if self.media.is_null() {
                bail!("libVLC can't open it: {}", self.vlc.last_error());
            }
            for option in &options {
                (api.libvlc_media_add_option)(self.media, option.as_ptr());
            }
            (api.libvlc_media_player_set_media)(self.player, self.media);
            if (api.libvlc_media_player_play)(self.player) != 0 {
                bail!("libVLC can't start it: {}", self.vlc.last_error());
            }
        }
        Ok(())
    }

    /// How far the conversion got, 0 to 1: the time read over the length, as audio-only
    /// sources leave the position at 0, else the position.
    fn progress(&self) -> f32 {
        let api = &self.vlc.api;
        // SAFETY: the player is live.
        let (time, length, position) = unsafe {
            (
                (api.libvlc_media_player_get_time)(self.player),
                (api.libvlc_media_player_get_length)(self.player),
                (api.libvlc_media_player_get_position)(self.player),
            )
        };
        let fraction = if length > 0 && time >= 0 { time as f32 / length as f32 } else { position };
        fraction.clamp(0., 1.)
    }

    /// What libVLC said went wrong, or a plain cause when it said nothing.
    fn error(&self) -> String {
        let said = self.vlc.last_error();
        if said.trim().is_empty() { tr!("VLC couldn't read or encode it").to_owned() } else { said }
    }

    /// The kinds of stream the played media has.
    fn streams(&self) -> Streams {
        let api = &self.vlc.api;
        let mut list: *mut *mut MediaTrack = std::ptr::null_mut();
        // SAFETY: the media is live and was played, so it is parsed; `list` receives `count`
        // tracks, released right after reading.
        unsafe {
            let count = (api.libvlc_media_tracks_get)(self.media, &raw mut list);
            if list.is_null() {
                return Streams::default();
            }
            let tracks = std::slice::from_raw_parts(list, count as usize);
            let has = |kind| tracks.iter().any(|&track| (*track).kind == kind);
            let streams = Streams { audio: has(TRACK_AUDIO), video: has(TRACK_VIDEO) };
            (api.libvlc_media_tracks_release)(list, count);
            streams
        }
    }
}

impl Drop for Run<'_> {
    fn drop(&mut self) {
        let api = &self.vlc.api;
        // SAFETY: each handle is live and released once; `stop` joins libVLC's threads, so the
        // file is finished and no callback runs after it, and the sender may then go.
        unsafe {
            (api.libvlc_media_player_stop)(self.player);
            (api.libvlc_media_player_release)(self.player);
            if !self.media.is_null() {
                (api.libvlc_media_release)(self.media);
            }
            (api.libvlc_release)(self.instance);
        }
    }
}

/// libVLC's event callback: says whether the conversion reached the end or failed. Never calls
/// libVLC.
unsafe extern "C" fn on_event(event: *const Event, data: *mut c_void) {
    // SAFETY: libVLC passes a live event, and `data` is the boxed sender the run owns until
    // after it releases the player.
    let (kind, ended) = unsafe { ((*event).kind, &*data.cast::<mpsc::Sender<bool>>()) };
    // Once the run is over, nobody listens.
    let _ = ended.send(kind == END_REACHED);
}
