//! [`AudioEffects`] as libVLC instance arguments: one `--audio-filter` chain (`normvol`,
//! `compressor`, `headphone`, `stereo_widen`), the normalizer's level, replay gain, the
//! equalizer's two passes and binaural rendering. Effects left off add nothing.

use crate::engine::audio::effects::{AudioEffects, ReplayGain, SpatialMode};

/// The arguments `effects` add.
pub fn arguments(effects: &AudioEffects) -> Vec<String> {
    let mut arguments = Vec::new();
    let filters = filters(effects);
    if !filters.is_empty() {
        arguments.push(format!("--audio-filter={}", filters.join(":")));
    }
    if let Some(level) = effects.normalize {
        arguments.push(format!("--norm-max-level={level}"));
    }
    if effects.replay_gain != ReplayGain::Off {
        arguments.push(format!("--audio-replay-gain-mode={}", effects.replay_gain.libvlc_name()));
        arguments.push(format!("--audio-replay-gain-preamp={}", effects.replay_gain_preamp));
    }
    if effects.equalizer_two_pass {
        arguments.push("--equalizer-2pass".to_owned());
    }
    if effects.spatial == Some(SpatialMode::Binaural) {
        arguments.push("--spatialaudio-headphones".to_owned());
    }
    arguments
}

/// The audio filters `effects` turn on, in the order sound goes through them.
fn filters(effects: &AudioEffects) -> Vec<&'static str> {
    let wanted = [
        (effects.normalize.is_some(), "normvol"),
        (effects.night_mode, "compressor"),
        (effects.spatial == Some(SpatialMode::HeadphoneVirtualization), "headphone"),
        (effects.stereo_widening, "stereo_widen"),
    ];
    wanted.into_iter().filter(|(on, _)| *on).map(|(_, filter)| filter).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_effects_add_nothing() {
        assert_eq!(arguments(&AudioEffects::default()), Vec::<String>::new());
    }

    #[test]
    fn effects_share_one_filter_chain() {
        let effects = AudioEffects {
            normalize: Some(2.5),
            night_mode: true,
            spatial: Some(SpatialMode::HeadphoneVirtualization),
            stereo_widening: true,
            ..AudioEffects::default()
        };
        assert_eq!(
            arguments(&effects),
            ["--audio-filter=normvol:compressor:headphone:stereo_widen", "--norm-max-level=2.5"]
        );
    }

    #[test]
    fn replay_gain_two_pass_and_binaural_are_their_own_options() {
        let effects = AudioEffects {
            replay_gain: ReplayGain::Album,
            replay_gain_preamp: -3.,
            equalizer_two_pass: true,
            spatial: Some(SpatialMode::Binaural),
            ..AudioEffects::default()
        };
        assert_eq!(
            arguments(&effects),
            [
                "--audio-replay-gain-mode=album",
                "--audio-replay-gain-preamp=-3",
                "--equalizer-2pass",
                "--spatialaudio-headphones",
            ]
        );
    }
}

#[cfg(test)]
mod libvlc_tests {
    use super::*;
    use crate::engine::libvlc::{ARGUMENTS, library, new_instance, runtime};

    #[test]
    #[ignore = "needs libVLC"]
    fn libvlc_takes_every_effect_option() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { runtime::prepare_environment() };
        let vlc = library().expect("libVLC loads");
        let effects = AudioEffects {
            normalize: Some(2.),
            night_mode: true,
            replay_gain: ReplayGain::Track,
            replay_gain_preamp: 1.5,
            equalizer_two_pass: true,
            spatial: Some(SpatialMode::Binaural),
            stereo_widening: true,
        };
        let options = arguments(&effects);
        let all: Vec<&str> =
            ARGUMENTS.iter().copied().chain(options.iter().map(String::as_str)).collect();
        let instance = new_instance(&vlc, &all).expect("libVLC takes the options");
        // SAFETY: the instance is live, unused and released once.
        unsafe { (vlc.api.libvlc_release)(instance) };
    }
}
