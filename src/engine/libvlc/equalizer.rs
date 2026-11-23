//! The equalizer on the worker thread (S5.8.2, S7.7): libVLC copies an equalizer's gains into
//! the player, which keeps them for every media it plays until another is set, and `NULL` turns
//! it off. The change is heard at once.
//!
//! **2 Pass.** libVLC's API has no call for it: its equalizer filter reads `equalizer-2pass`
//! when it opens, from its audio output, which inherits it from the player. Setting an equalizer
//! always reopens the filter (libVLC rewrites the output's `audio-filter`), so the variable is
//! set on the player and on its running audio output first, where the output keeps the value
//! its first filter read.
//!
//! libVLC's call also sets the output's filter chain to the equalizer alone, so the chain with
//! the other effects is written again after it (`audio_effects.rs`).

use std::ffi::CStr;
use std::ptr;

use super::api::AudioEqualizer;
use super::audio_effects::AUDIO_OUTPUT;
use super::core_vars::Var;
use super::worker::Worker;
use crate::engine::audio::equalizer::{Equalization, Equalizer};

/// libVLC's 2 Pass switch.
const TWO_PASS: &CStr = c"equalizer-2pass";

impl Worker {
    /// Plays through `equalization` from now on, or without an equalizer.
    pub(super) fn set_equalizer(&mut self, equalization: Option<&Equalization>) {
        if let Some(equalization) = equalization {
            self.set_two_pass(equalization.two_pass);
        }
        if let Err(problem) = self.try_set_equalizer(equalization.map(|eq| &eq.gains)) {
            tracing::warn!("libvlc: {problem}");
        }
        self.equalizer_on = equalization.is_some();
        self.write_audio_filters(false);
    }

    /// Sets `equalizer-2pass` on the player and its audio output, for the filter to read when
    /// it next opens.
    fn set_two_pass(&self, on: bool) {
        let player = self.handles.player;
        // SAFETY: the player is live for the worker's life.
        unsafe {
            self.vlc.set_player_var(player, TWO_PASS, Var::Bool(on));
            self.vlc.set_child_var(player, AUDIO_OUTPUT, TWO_PASS, Var::Bool(on));
        }
    }

    fn try_set_equalizer(&self, equalizer: Option<&Equalizer>) -> Result<(), &'static str> {
        let made = match equalizer {
            Some(equalizer) => self.make_equalizer(equalizer),
            None => ptr::null_mut(),
        };
        if equalizer.is_some() && made.is_null() {
            return Err("couldn't make an equalizer");
        }
        if self.give_equalizer(made) { Ok(()) } else { Err("the player refused the equalizer") }
    }

    /// Hands `made` to the player, which copies it (`NULL` turns its equalizer off), then
    /// releases it. Returns whether the player took it.
    fn give_equalizer(&self, made: *mut AudioEqualizer) -> bool {
        let api = &self.vlc.api;
        // SAFETY: the player is live; `made` is null or ours, released once after the copy.
        unsafe {
            let taken = (api.libvlc_media_player_set_equalizer)(self.handles.player, made) == 0;
            if !made.is_null() {
                (api.libvlc_audio_equalizer_release)(made);
            }
            taken
        }
    }

    /// A libVLC equalizer with `equalizer`'s gains, for the caller to release; null if libVLC
    /// couldn't make one.
    fn make_equalizer(&self, equalizer: &Equalizer) -> *mut AudioEqualizer {
        let api = &self.vlc.api;
        // SAFETY: a new equalizer, set up while nothing else holds it.
        unsafe {
            let made = (api.libvlc_audio_equalizer_new)();
            if !made.is_null() {
                (api.libvlc_audio_equalizer_set_preamp)(made, equalizer.preamp);
                for (index, gain) in (0..).zip(equalizer.bands) {
                    (api.libvlc_audio_equalizer_set_amp_at_index)(made, gain, index);
                }
            }
            made
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::time::{Duration, Instant};

    use super::super::core_vars::Var;
    use super::{AUDIO_OUTPUT, TWO_PASS};
    use crate::engine::audio::equalizer::Preset;

    /// Gains libVLC stores as -1.11022e-15 where its table means 0.
    const TOLERANCE: f32 = 1e-3;

    #[test]
    #[ignore = "needs libVLC"]
    fn the_presets_match_libvlcs() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { super::super::runtime::prepare_environment() };
        let vlc = super::super::library().expect("libVLC loads");
        let api = &vlc.api;
        // SAFETY: plain reads of libVLC's preset table, each equalizer released once.
        unsafe {
            assert_eq!((api.libvlc_audio_equalizer_get_preset_count)() as usize, Preset::ALL.len());
            for (index, preset) in (0..).zip(Preset::ALL) {
                let made = (api.libvlc_audio_equalizer_new_from_preset)(index);
                assert!(!made.is_null(), "{preset:?}");
                let ours = preset.equalizer().expect("a preset has gains");
                let preamp = (api.libvlc_audio_equalizer_get_preamp)(made);
                assert!((preamp - ours.preamp).abs() < TOLERANCE, "{preset:?} preamp {preamp}");
                for (band, want) in (0..).zip(ours.bands) {
                    let gain = (api.libvlc_audio_equalizer_get_amp_at_index)(made, band);
                    assert!((gain - want).abs() < TOLERANCE, "{preset:?} band {band}: {gain}");
                }
                (api.libvlc_audio_equalizer_release)(made);
            }
        }
    }

    #[test]
    #[ignore = "needs libVLC and an audio output"]
    fn two_pass_reaches_the_running_audio_output() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { super::super::runtime::prepare_environment() };
        let vlc = super::super::library().expect("libVLC loads");
        let instance = super::super::new_instance(&vlc, &super::super::ARGUMENTS).expect("one");
        let api = &vlc.api;
        let tone = super::super::test_tone::tone_file().expect("the tone");
        let path = CString::new(tone.to_str().expect("a UTF-8 path")).expect("no NUL");
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
                assert!(started.elapsed() < Duration::from_secs(5), "the tone never played");
                std::thread::sleep(Duration::from_millis(20));
            }
            for on in [true, false, true] {
                vlc.set_player_var(player, TWO_PASS, Var::Bool(on));
                vlc.set_child_var(player, AUDIO_OUTPUT, TWO_PASS, Var::Bool(on));
                let flat = (api.libvlc_audio_equalizer_new_from_preset)(0);
                assert_eq!((api.libvlc_media_player_set_equalizer)(player, flat), 0);
                (api.libvlc_audio_equalizer_release)(flat);
                assert_eq!(vlc.child_bool(player, AUDIO_OUTPUT, TWO_PASS), Some(on));
            }
            (api.libvlc_media_player_stop)(player);
            (api.libvlc_media_player_release)(player);
            (api.libvlc_release)(instance);
        }
    }
}
