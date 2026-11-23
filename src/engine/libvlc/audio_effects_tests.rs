//! A libVLC smoke test of the audio effects (S7.9): the chain and the settings reach the running
//! audio output, which keeps them.

use std::ffi::CString;
use std::time::{Duration, Instant};

use super::super::{ARGUMENTS, library, new_instance, runtime, test_tone};
use super::*;

#[test]
#[ignore = "needs libVLC and an audio output"]
fn the_effects_reach_the_running_audio_output() {
    // SAFETY: run on its own, as the other libVLC smoke tests.
    unsafe { runtime::prepare_environment() };
    let vlc = library().expect("libVLC loads");
    let instance = new_instance(&vlc, &ARGUMENTS).expect("an instance");
    let api = &vlc.api;
    let tone = test_tone::tone_file().expect("the tone");
    let path = CString::new(tone.to_str().expect("a UTF-8 path")).expect("no NUL");
    let effects = AudioEffects {
        compressor: Some(Compressor::NIGHT),
        widener: Some(StereoWidener::DEFAULT),
        spatial: Some(SpatialMode::Binaural),
        normalize: Some(3.),
        ..AudioEffects::default()
    };
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
        for (name, value) in settings(&effects) {
            set_audio_var(&vlc, player, name, value);
        }
        let chain = chain(&effects, false);
        set_audio_var(&vlc, player, AUDIO_FILTER, Var::String(c""));
        set_audio_var(&vlc, player, AUDIO_FILTER, Var::String(&chain));
        let filters = vlc.child_string(player, AUDIO_OUTPUT, AUDIO_FILTER);
        assert_eq!(filters.as_deref(), Some("compressor:stereo_widen:normvol"));
        let binaural = vlc.child_bool(player, AUDIO_OUTPUT, c"spatialaudio-headphones");
        assert_eq!(binaural, Some(true));
        std::thread::sleep(Duration::from_millis(300));
        assert!((api.libvlc_media_player_get_time)(player) > 0, "it plays on");
        (api.libvlc_media_player_stop)(player);
        (api.libvlc_media_player_release)(player);
        (api.libvlc_release)(instance);
    }
}
