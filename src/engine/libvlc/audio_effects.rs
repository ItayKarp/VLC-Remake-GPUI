//! The audio effects on the worker thread (S7.9): one `audio-filter` chain (`equalizer`,
//! `compressor`, `spatializer`, `stereo_widen`, `headphone`, `normvol`, in the order sound goes
//! through them) and each filter's settings, set on the player, which an audio output opened
//! later inherits, and on its running audio output.
//!
//! libVLC's API has no call for them. The output restarts its filters when its `audio-filter`
//! changes, a moment's gap, so the chain is written only when an effect goes on or off. The
//! compressor, spatializer and widener follow their variables while they run, so a slider is
//! heard at once without a restart; replay gain is the output's own and follows too. The
//! normalizer and binaural rendering read theirs only when they open, so changing either writes
//! another chain first, which restarts the filters. libVLC's equalizer call rewrites the chain
//! as `equalizer` alone, so the chain is written again after it (`equalizer.rs`).

use std::ffi::{CStr, CString};

use super::api::{LibVlc, MediaPlayer};
use super::core_vars::Var;
use super::worker::Worker;
use crate::engine::audio::effects::{
    AudioEffects, Compressor, SpatialMode, Spatializer, StereoWidener,
};

/// The audio output's object type and its filter chain.
pub(super) const AUDIO_OUTPUT: &CStr = c"audio output";
const AUDIO_FILTER: &CStr = c"audio-filter";
/// The gain a file without replay gain tags gets: none, so nothing is boosted into clipping.
const UNTAGGED_GAIN: f32 = 0.;

/// A filter's setting: its variable and value.
type Setting = (&'static CStr, Var<'static>);

impl Worker {
    /// Plays through `effects` from now on, for this and later media.
    pub(super) fn set_audio_effects(&mut self, effects: AudioEffects) {
        let before = std::mem::replace(&mut self.audio_effects, effects);
        for (name, value) in settings(&effects) {
            self.set_audio_var(name, value);
        }
        self.write_audio_filters(reopens(&before, &effects));
    }

    /// Writes the filter chain the effects and the equalizer make, restarting the filters
    /// first if `reopen`.
    pub(super) fn write_audio_filters(&self, reopen: bool) {
        let chain = chain(&self.audio_effects, self.equalizer_on);
        if reopen {
            // A different chain, so the output restarts even if this one is the same.
            let other = if chain.is_empty() { c":" } else { c"" };
            self.set_audio_var(AUDIO_FILTER, Var::String(other));
        }
        self.set_audio_var(AUDIO_FILTER, Var::String(&chain));
    }

    /// Sets `name` on the player and its running audio output.
    fn set_audio_var(&self, name: &CStr, to: Var) {
        // SAFETY: the player is live for the worker's life.
        unsafe { set_audio_var(&self.vlc, self.handles.player, name, to) };
    }
}

/// Sets `name` on `player` and its running audio output.
///
/// # Safety
///
/// `player` is a live `libvlc_media_player_t`.
unsafe fn set_audio_var(vlc: &LibVlc, player: *mut MediaPlayer, name: &CStr, to: Var) {
    // SAFETY: the caller's promise.
    unsafe {
        vlc.set_player_var(player, name, to);
        vlc.set_child_var(player, AUDIO_OUTPUT, name, to);
    }
}

/// The filters `effects` and the equalizer turn on, in the order sound goes through them.
fn filters(effects: &AudioEffects, equalizer: bool) -> Vec<&'static str> {
    let headphone = effects.spatial == Some(SpatialMode::HeadphoneVirtualization);
    let wanted = [
        (equalizer, "equalizer"),
        (effects.compressor.is_some(), "compressor"),
        (effects.spatializer.is_some(), "spatializer"),
        (effects.widener.is_some(), "stereo_widen"),
        (headphone, "headphone"),
        (effects.normalize.is_some(), "normvol"),
    ];
    wanted.into_iter().filter(|(on, _)| *on).map(|(_, filter)| filter).collect()
}

/// The `audio-filter` value.
fn chain(effects: &AudioEffects, equalizer: bool) -> CString {
    CString::new(filters(effects, equalizer).join(":")).expect("filter names have no NUL")
}

/// Whether going from `before` to `after` changes a setting read only when a filter opens.
fn reopens(before: &AudioEffects, after: &AudioEffects) -> bool {
    let binaural = |effects: &AudioEffects| effects.spatial == Some(SpatialMode::Binaural);
    before.normalize != after.normalize || binaural(before) != binaural(after)
}

/// Every variable `effects` sets: the settings of the filters on, replay gain and binaural
/// rendering.
fn settings(effects: &AudioEffects) -> Vec<Setting> {
    let mut settings = vec![
        (c"audio-replay-gain-mode", Var::String(effects.replay_gain.libvlc_name())),
        (c"audio-replay-gain-preamp", Var::Float(effects.replay_gain_preamp)),
        (c"audio-replay-gain-default", Var::Float(UNTAGGED_GAIN)),
        (c"audio-replay-gain-peak-protection", Var::Bool(true)),
        (c"spatialaudio-headphones", Var::Bool(effects.spatial == Some(SpatialMode::Binaural))),
    ];
    settings.extend(effects.compressor.map(compressor).into_iter().flatten());
    settings.extend(effects.spatializer.map(spatializer).into_iter().flatten());
    settings.extend(effects.widener.map(widener).into_iter().flatten());
    settings.extend(effects.normalize.map(|level| (c"norm-max-level", Var::Float(level))));
    settings
}

fn compressor(compressor: Compressor) -> [Setting; 7] {
    let c = compressor.clamped();
    [
        (c"compressor-rms-peak", Var::Float(c.rms_peak)),
        (c"compressor-attack", Var::Float(c.attack_ms)),
        (c"compressor-release", Var::Float(c.release_ms)),
        (c"compressor-threshold", Var::Float(c.threshold_db)),
        (c"compressor-ratio", Var::Float(c.ratio)),
        (c"compressor-knee", Var::Float(c.knee_db)),
        (c"compressor-makeup-gain", Var::Float(c.makeup_db)),
    ]
}

fn spatializer(spatializer: Spatializer) -> [Setting; 5] {
    let s = spatializer.clamped();
    [
        (c"spatializer-roomsize", Var::Float(s.size)),
        (c"spatializer-width", Var::Float(s.width)),
        (c"spatializer-wet", Var::Float(s.wet)),
        (c"spatializer-dry", Var::Float(s.dry)),
        (c"spatializer-damp", Var::Float(s.damp)),
    ]
}

fn widener(widener: StereoWidener) -> [Setting; 4] {
    let w = widener.clamped();
    [
        (c"stereowiden-delay", Var::Float(w.delay_ms)),
        (c"stereowiden-feedback", Var::Float(w.feedback)),
        (c"stereowiden-crossfeed", Var::Float(w.crossfeed)),
        (c"stereowiden-dry-mix", Var::Float(w.dry_mix)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(settings: &[Setting]) -> Vec<&str> {
        settings.iter().map(|(name, _)| name.to_str().expect("ASCII")).collect()
    }

    #[test]
    fn no_effects_make_an_empty_chain() {
        assert_eq!(chain(&AudioEffects::default(), false), c"");
        assert_eq!(chain(&AudioEffects::default(), true), c"equalizer");
    }

    #[test]
    fn the_chain_keeps_the_documented_order() {
        let effects = AudioEffects {
            compressor: Some(Compressor::DEFAULT),
            spatializer: Some(Spatializer::DEFAULT),
            widener: Some(StereoWidener::DEFAULT),
            spatial: Some(SpatialMode::HeadphoneVirtualization),
            normalize: Some(2.),
            ..AudioEffects::default()
        };
        assert_eq!(
            chain(&effects, true),
            c"equalizer:compressor:spatializer:stereo_widen:headphone:normvol"
        );
    }

    #[test]
    fn only_the_filters_on_get_settings() {
        let off = settings(&AudioEffects::default());
        assert!(!names(&off).iter().any(|name| name.starts_with("compressor")));
        let on = AudioEffects { widener: Some(StereoWidener::DEFAULT), ..AudioEffects::default() };
        let settings = settings(&on);
        let names = names(&settings);
        assert!(names.contains(&"stereowiden-dry-mix"));
        assert!(names.contains(&"audio-replay-gain-mode"), "replay gain is always set");
    }

    #[test]
    fn only_open_time_settings_restart_the_filters() {
        let quiet = AudioEffects { normalize: Some(2.), ..AudioEffects::default() };
        let louder = AudioEffects { normalize: Some(4.), ..quiet };
        assert!(reopens(&quiet, &louder));
        let compressed = AudioEffects { compressor: Some(Compressor::NIGHT), ..quiet };
        let harder = AudioEffects { compressor: Some(Compressor::DEFAULT), ..quiet };
        assert!(!reopens(&compressed, &harder), "the compressor follows while it runs");
        let binaural = AudioEffects { spatial: Some(SpatialMode::Binaural), ..quiet };
        assert!(reopens(&quiet, &binaural));
    }
}

#[cfg(test)]
#[path = "audio_effects_tests.rs"]
mod libvlc_tests;
