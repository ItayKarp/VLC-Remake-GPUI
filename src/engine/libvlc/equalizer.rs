//! The equalizer on the worker thread (S5.8.2): libVLC copies an equalizer's gains into the
//! player, which keeps them for every media it plays until another is set, and `NULL` turns it
//! off. The change is heard at once.

use std::ptr;

use super::api::AudioEqualizer;
use super::worker::Worker;
use crate::engine::audio::equalizer::Equalizer;

impl Worker {
    /// Plays through `equalizer` from now on, or without one.
    pub(super) fn set_equalizer(&self, equalizer: Option<&Equalizer>) {
        if let Err(problem) = self.try_set_equalizer(equalizer) {
            tracing::warn!("libvlc: {problem}");
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
}
