//! The Stereo Mode's Test on the worker thread (S7.6.2): a second media player, made at the
//! first test and kept, plays the test tone ([`crate::engine::audio::test_tone`]) through the
//! output device and channels in use, at a fixed volume, beside whatever the main player plays.
//! The tone is written once to the temporary folder for libVLC to open.

use std::ffi::{CString, c_int};
use std::path::PathBuf;

use super::api::MediaPlayer;
use super::audio::{set_channels, set_device};
use super::worker::Worker;
use crate::engine::audio::{OutputDevice, test_tone};

/// The tone's file in the temporary folder.
const FILE_NAME: &str = "rvlc-test-tone.wav";
/// The test player's volume: libVLC's 100%, whatever the media's volume or boost is.
const VOLUME: c_int = 100;

/// The test's own player and where it plays.
#[derive(Debug)]
pub struct TestTone {
    /// Its player; null until the first test.
    player: *mut MediaPlayer,
    /// The device the main player plays through.
    pub device: OutputDevice,
}

// SAFETY: a libVLC player is a thread-safe object; only the worker thread uses this one.
unsafe impl Send for TestTone {}

impl Default for TestTone {
    fn default() -> Self {
        Self { player: std::ptr::null_mut(), device: OutputDevice::SystemDefault }
    }
}

impl Worker {
    /// Plays the test tone from its start, through the device and channels in use.
    pub(super) fn play_test_tone(&mut self) {
        let Some(path) = tone_file().and_then(|path| CString::new(path.to_str()?).ok()) else {
            return;
        };
        let Some(player) = self.test_player() else { return };
        set_device(&self.vlc, player, &self.test_tone.device);
        let _ = set_channels(&self.vlc, player, self.channels);
        if !self.load_tone(player, &path) {
            return;
        }
        let api = &self.vlc.api;
        // SAFETY: the test player is live.
        unsafe {
            _ = (api.libvlc_audio_set_volume)(player, VOLUME);
            if (api.libvlc_media_player_play)(player) != 0 {
                tracing::warn!("libvlc: couldn't play the test tone");
            }
        }
    }

    /// Stops the test's `player` and gives it the tone at `path`; `false` if it won't open.
    fn load_tone(&self, player: *mut MediaPlayer, path: &CString) -> bool {
        let api = &self.vlc.api;
        // SAFETY: the instance and the test player are live; `path` is NUL-terminated, and the
        // media's reference is handed to the player before ours is released.
        unsafe {
            (api.libvlc_media_player_stop)(player);
            let media = (api.libvlc_media_new_path)(self.handles.instance, path.as_ptr());
            if media.is_null() {
                tracing::warn!("libvlc: couldn't open the test tone: {}", self.vlc.last_error());
                return false;
            }
            (api.libvlc_media_player_set_media)(player, media);
            (api.libvlc_media_release)(media);
        }
        true
    }

    /// Stops and releases the test's player, if one was made.
    pub(super) fn release_test_tone(&mut self) {
        let player = std::mem::replace(&mut self.test_tone.player, std::ptr::null_mut());
        if player.is_null() {
            return;
        }
        let api = &self.vlc.api;
        // SAFETY: the test player is live and released once, here.
        unsafe {
            (api.libvlc_media_player_stop)(player);
            (api.libvlc_media_player_release)(player);
        }
    }

    /// The test's player, made the first time.
    fn test_player(&mut self) -> Option<*mut MediaPlayer> {
        if self.test_tone.player.is_null() {
            // SAFETY: the instance is live; the player is released in `release_test_tone`.
            self.test_tone.player =
                unsafe { (self.vlc.api.libvlc_media_player_new)(self.handles.instance) };
        }
        let player = self.test_tone.player;
        if player.is_null() {
            tracing::warn!("libvlc: no player for the test tone: {}", self.vlc.last_error());
        }
        (!player.is_null()).then_some(player)
    }
}

/// The tone's file, written unless it is already there.
fn tone_file() -> Option<PathBuf> {
    let path = std::env::temp_dir().join(FILE_NAME);
    let wav = test_tone::wav();
    let written = std::fs::metadata(&path).is_ok_and(|file| file.len() == wav.len() as u64);
    if !written && let Err(error) = std::fs::write(&path, &wav) {
        tracing::warn!("libvlc: couldn't write the test tone: {error}");
        return None;
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::engine::audio::StereoMode;
    use crate::engine::libvlc::audio::Channels;
    use crate::engine::libvlc::{ARGUMENTS, library, new_instance, runtime};

    /// How long the tone may take to start playing.
    const START: Duration = Duration::from_secs(5);

    #[test]
    #[ignore = "needs libVLC and an audio output"]
    fn every_stereo_mode_holds_while_the_tone_plays() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { runtime::prepare_environment() };
        let vlc = library().expect("libVLC loads");
        let instance = new_instance(&vlc, &ARGUMENTS).expect("an instance");
        let api = &vlc.api;
        let path = tone_file().and_then(|path| CString::new(path.to_str()?).ok()).expect("tone");
        // SAFETY: the instance and player are live until released at the end, once each; the
        // media's reference goes to the player before ours is released.
        unsafe {
            let player = (api.libvlc_media_player_new)(instance);
            let media = (api.libvlc_media_new_path)(instance, path.as_ptr());
            (api.libvlc_media_player_set_media)(player, media);
            (api.libvlc_media_release)(media);
            assert_eq!((api.libvlc_media_player_play)(player), 0);
            let started = Instant::now();
            while (api.libvlc_media_player_get_time)(player) <= 0 {
                assert!(started.elapsed() < START, "the tone never played");
                std::thread::sleep(Duration::from_millis(20));
            }
            for stereo in [
                StereoMode::Mono,
                StereoMode::Left,
                StereoMode::Right,
                StereoMode::ReverseStereo,
                StereoMode::DolbySurround,
                StereoMode::Headphones,
            ] {
                let channels = Channels { stereo, ..Channels::default() };
                assert!(set_channels(&vlc, player, channels), "{stereo:?}");
                std::thread::sleep(Duration::from_millis(100));
                let held = (api.libvlc_audio_get_channel)(player);
                assert_eq!(held, channels.stereo_mode(), "{stereo:?}");
            }
            (api.libvlc_media_player_stop)(player);
            (api.libvlc_media_player_release)(player);
            (api.libvlc_release)(instance);
        }
    }
}
