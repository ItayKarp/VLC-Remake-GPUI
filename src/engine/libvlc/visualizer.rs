//! The visualizer on the worker thread (S7.8.1): libVLC's `visual` audio filter draws
//! Spectrometer, Scope or Spectrum into a video output of its own, which `vmem` hands to the
//! video area like any picture.
//!
//! libVLC's API has no call for it. Its audio output has a `visual` variable whose callback
//! swaps the filter in or out of the running output, heard and seen at once; an output opened
//! later reads `audio-visual` and `effect-list`, inherited from the player. Both are set.
//!
//! An audio decoder starting drops a visualizer given before it, so it is given again once the
//! media's time first moves after an open, a reopen or an audio track switch
//! ([`Worker::restart_visualizer`]); that restarts the output, a moment's gap, and its device
//! list is left unread meanwhile, as after a reopen.
//!
//! Only media without video shows it: a second video output would reach the one `vmem` the
//! video plays through. So it is switched off before any media opens, and on once playback
//! started and its streams say there is no video.

use std::ffi::{CStr, CString};
use std::time::Instant;

use super::core_vars::Var;
use super::worker::Worker;
use crate::engine::audio::visualizer::VisualEffect;

/// The audio output's switch, its object type, and what an output opened later reads.
const VISUAL: &CStr = c"visual";
const AUDIO_OUTPUT: &CStr = c"audio output";
const AUDIO_VISUAL: &CStr = c"audio-visual";
const EFFECT_LIST: &CStr = c"effect-list";
/// The module drawing the effects, and the name of none.
const VISUAL_MODULE: &CStr = c"visual";
const NO_MODULE: &CStr = c"none";

/// The visualizer wanted, and the one the audio output was given.
#[derive(Clone, Copy, Debug, Default)]
pub struct Visuals {
    wanted: Option<VisualEffect>,
    shown: Option<VisualEffect>,
}

impl Worker {
    /// Draws `effect` for this and later media without video, or nothing.
    pub(super) fn set_visualizer(&mut self, effect: Option<VisualEffect>) {
        self.visuals.wanted = effect;
        self.apply_visualizer();
    }

    /// Draws the wanted visualizer if the media playing has no video; stops drawing it if
    /// it does.
    pub(super) fn apply_visualizer(&mut self) {
        let playing = !self.media.is_null();
        let effect = self.visuals.wanted.filter(|_| playing && !self.has_video_stream());
        self.show_visual(effect);
    }

    /// Gives the audio output the wanted visualizer again, now that its audio decodes.
    pub(super) fn restart_visualizer(&mut self) {
        self.visuals.shown = None;
        self.apply_visualizer();
    }

    /// Stops drawing before other media opens, whose streams aren't known yet.
    pub(super) fn stop_visualizer(&mut self) {
        self.show_visual(None);
    }

    fn show_visual(&mut self, effect: Option<VisualEffect>) {
        if self.visuals.shown == effect {
            return;
        }
        self.visuals.shown = effect;
        let name = effect.map_or("", VisualEffect::name);
        let Ok(name) = CString::new(name) else { return };
        let module = if effect.is_some() { VISUAL_MODULE } else { NO_MODULE };
        let player = self.handles.player;
        // SAFETY: the player is live for the worker's life.
        unsafe {
            self.vlc.set_player_var(player, AUDIO_VISUAL, Var::String(module));
            self.vlc.set_player_var(player, EFFECT_LIST, Var::String(&name));
            self.vlc.set_child_var(player, AUDIO_OUTPUT, VISUAL, Var::String(&name));
        }
        // The output restarts, listing no devices meanwhile, which would read as the device
        // going away and pause.
        self.devices.settle(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CString;
    use std::time::{Duration, Instant};

    use super::super::core_vars::Var;
    use super::{AUDIO_OUTPUT, VISUAL};

    #[test]
    #[ignore = "needs libVLC and an audio output"]
    fn the_running_audio_output_takes_the_visualizer() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { super::super::runtime::prepare_environment() };
        let vlc = super::super::library().expect("libVLC loads");
        let arguments =
            [super::super::ARGUMENTS.as_slice(), &[super::super::instance::NO_VIDEO_ARGUMENT]];
        let instance = super::super::new_instance(&vlc, &arguments.concat()).expect("one");
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
            for effect in [c"spectrum", c"", c"scope"] {
                vlc.set_child_var(player, AUDIO_OUTPUT, VISUAL, Var::String(effect));
                let held = vlc.child_string(player, AUDIO_OUTPUT, c"audio-visual");
                let want = if effect.is_empty() { "none" } else { "visual" };
                assert_eq!(held.as_deref(), Some(want), "{effect:?}");
            }
            (api.libvlc_media_player_stop)(player);
            (api.libvlc_media_player_release)(player);
            (api.libvlc_release)(instance);
        }
    }

    #[test]
    #[ignore = "needs libVLC and an audio output"]
    fn media_without_video_shows_the_visualizers_pictures() {
        use std::sync::Arc;
        use std::sync::atomic::Ordering;

        use super::super::LibVlcBackend;
        use super::super::video_tests::{CountingSink, PATIENCE};
        use crate::app::media::Media;
        use crate::engine::audio::AudioConfig;
        use crate::engine::audio::visualizer::VisualEffect;
        use crate::engine::backend::{self, Backend};
        use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
        use crate::engine::subtitle_style::SubtitleStyle;
        use crate::engine::values::{Generation, Start};

        /// Pictures to wait for: the tone is short, and the visualizer starts once it sounds.
        const PICTURES: usize = 3;

        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { super::super::runtime::prepare_environment() };
        let tone = super::super::test_tone::tone_file().expect("the tone");
        let sink = Arc::new(CountingSink::default());
        let (events, _receiver) = backend::channel();
        let style = SubtitleStyle::default();
        let backend =
            LibVlcBackend::new(events, Some(sink.clone()), AudioConfig::default(), &style);
        let mut backend = Box::new(backend.expect("libVLC"));
        backend.set_visualizer(Some(VisualEffect::Spectrum));
        backend.open(&Media::File(tone), Start::default(), Generation::default().next());
        let deadline = Instant::now() + PATIENCE;
        while sink.displayed.load(Ordering::Relaxed) < PICTURES {
            assert!(Instant::now() < deadline, "the visualizer drew nothing");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
    }
}
