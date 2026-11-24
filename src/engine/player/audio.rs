//! The [`Player`]'s audio (S1.5): volume within the maximum, mute, the output device and its
//! hot-plug fallback, speaker setup and stereo mode (S7.6.2), the equalizer and the effects
//! after it (S7.9), the visualizer (S7.8.1), and pausing for another app's audio.

use std::time::{Duration, Instant};

use gpui::{Context, Task};

use super::{Intent, Player, PlayerEvent};
use crate::engine::audio::effects::AudioEffects;
use crate::engine::audio::equalizer::Equalization;
use crate::engine::audio::visualizer::VisualEffect;
use crate::engine::audio::{
    AudioDevice, AudioPolicy, OutputDevice, SpeakerSetup, StereoMode, hotplug,
};
use crate::engine::state::PlaybackState;
use crate::engine::values::{DEFAULT_MAX_VOLUME, MIN_MAX_VOLUME, Volume, VolumeChange};

/// How long after sound starts a device going away doesn't pause: AirPods leave a Mac about 3 s
/// after any app starts playing when their automatic switching or ear detection takes them
/// elsewhere, which would pause every play. Sound still moves to the default device.
pub(super) const DEVICE_LOSS_GRACE: Duration = Duration::from_secs(5);

/// The player's audio state, kept across opens.
pub(super) struct AudioState {
    /// The loudest volume allowed ("Maximum volume").
    max_volume: Volume,
    /// The device the user chose.
    preferred: OutputDevice,
    /// The device sound goes to: the preferred one, or the default while it is unplugged.
    active: OutputDevice,
    speaker_setup: SpeakerSetup,
    stereo_mode: StereoMode,
    /// The equalizer sound goes through; `None` while it is off.
    equalizer: Option<Equalization>,
    /// The effects after the equalizer.
    effects: AudioEffects,
    /// The visualizer drawn for media without video; `None` while there is none.
    visualizer: Option<VisualEffect>,
    policy: AudioPolicy,
    /// The devices plugged in; `None` until the backend reports them.
    devices: Option<Vec<AudioDevice>>,
    /// Whether the player paused for another app's audio, and resumes when it ends.
    paused_for_cork: bool,
    /// Whether the player paused for the Stereo Mode's test tone, and resumes after it.
    pub(super) paused_for_test: bool,
    /// Ends the test tone's pause once the tone is over (`test_tone.rs`).
    pub(super) test_ends: Option<Task<()>>,
    /// When playback last started or resumed, for [`DEVICE_LOSS_GRACE`].
    pub(super) sounding_since: Option<Instant>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self {
            max_volume: DEFAULT_MAX_VOLUME,
            preferred: OutputDevice::SystemDefault,
            active: OutputDevice::SystemDefault,
            speaker_setup: SpeakerSetup::Automatic,
            stereo_mode: StereoMode::Stereo,
            equalizer: None,
            effects: AudioEffects::default(),
            visualizer: None,
            policy: AudioPolicy::default(),
            devices: None,
            paused_for_cork: false,
            paused_for_test: false,
            test_ends: None,
            sounding_since: None,
        }
    }
}

impl AudioState {
    /// Forgets a pause for another app's audio or the test tone: the user took over playback.
    pub(super) fn user_took_over(&mut self) {
        self.paused_for_cork = false;
        self.paused_for_test = false;
    }
}

impl Player {
    /// The loudest volume allowed.
    pub fn max_volume(&self) -> Volume {
        self.audio.max_volume
    }

    /// The devices plugged in, without the system default; empty until reported, or when the
    /// machine has none.
    pub fn audio_devices(&self) -> &[AudioDevice] {
        self.audio.devices.as_deref().unwrap_or_default()
    }

    /// Whether the machine is known to have no output device: media plays silently.
    pub fn has_no_audio_device(&self) -> bool {
        self.audio.devices.as_ref().is_some_and(Vec::is_empty)
    }

    /// The device the user chose.
    pub fn preferred_audio_device(&self) -> &OutputDevice {
        &self.audio.preferred
    }

    /// The device sound goes to now.
    pub fn active_audio_device(&self) -> &OutputDevice {
        &self.audio.active
    }

    /// Sets the output volume, up to [`Self::max_volume`]. Changing the volume unmutes, as in
    /// VLC.
    pub fn set_volume(&mut self, volume: Volume, cx: &mut Context<Self>) {
        let volume = volume.min(self.audio.max_volume);
        let unmute = self.controls.muted;
        if volume == self.controls.volume && !unmute {
            return;
        }
        self.controls.volume = volume;
        self.push_volume();
        if unmute {
            self.controls.muted = false;
            self.with_backend(|backend| backend.set_mute(false));
        }
        cx.emit(PlayerEvent::VolumeChanged);
        cx.notify();
    }

    /// Raises or lowers the volume by `step` (Up/Down keys, the scroll wheel).
    pub fn step_volume(&mut self, change: VolumeChange, step: Volume, cx: &mut Context<Self>) {
        self.set_volume(self.controls.volume.stepped(change, step), cx);
    }

    /// Mutes or unmutes the output.
    pub fn set_mute(&mut self, muted: bool, cx: &mut Context<Self>) {
        if muted == self.controls.muted {
            return;
        }
        self.controls.muted = muted;
        self.with_backend(|backend| backend.set_mute(muted));
        cx.emit(PlayerEvent::VolumeChanged);
        cx.notify();
    }

    /// Mutes when unmuted, and the other way round.
    pub fn toggle_mute(&mut self, cx: &mut Context<Self>) {
        self.set_mute(!self.controls.muted, cx);
    }

    /// Sets the loudest volume allowed, 100% to 300%; a louder current volume comes down at
    /// once.
    pub fn set_max_volume(&mut self, max: Volume, cx: &mut Context<Self>) {
        let max = if max.get() < MIN_MAX_VOLUME.get() { MIN_MAX_VOLUME } else { max };
        self.audio.max_volume = max;
        if self.controls.volume.get() > max.get() {
            self.controls.volume = max;
            self.push_volume();
            cx.emit(PlayerEvent::VolumeChanged);
        }
        cx.notify();
    }

    /// Plays through `device`, or through the default while it is unplugged.
    pub fn set_audio_device(&mut self, device: OutputDevice, cx: &mut Context<Self>) {
        self.audio.preferred = device;
        self.reroute();
        cx.notify();
    }

    /// Lists the output devices again now (the track popover's refresh, S2.6.3); a changed list
    /// arrives as [`PlayerEvent::AudioDevicesChanged`].
    pub fn refresh_audio_devices(&mut self) {
        self.with_backend(|backend| backend.refresh_audio_devices());
    }

    /// Mixes channels for `setup`.
    pub fn set_speaker_setup(&mut self, setup: SpeakerSetup, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.audio.speaker_setup, setup) != setup {
            self.with_backend(|backend| backend.set_speaker_setup(setup));
            cx.notify();
        }
    }

    /// How the two channels play.
    pub fn stereo_mode(&self) -> StereoMode {
        self.audio.stereo_mode
    }

    /// Plays the two channels as `mode` says; heard at once.
    pub fn set_stereo_mode(&mut self, mode: StereoMode, cx: &mut Context<Self>) {
        if std::mem::replace(&mut self.audio.stereo_mode, mode) != mode {
            self.with_backend(|backend| backend.set_stereo_mode(mode));
            cx.notify();
        }
    }

    /// Plays through `equalizer` from now on, or without one; heard at once.
    pub fn set_equalizer(&mut self, equalizer: Option<Equalization>) {
        if self.audio.equalizer != equalizer {
            self.audio.equalizer = equalizer;
            self.with_backend(|backend| backend.set_equalizer(equalizer));
        }
    }

    /// Plays through `effects` from now on; heard at once.
    pub fn set_audio_effects(&mut self, effects: AudioEffects) {
        if std::mem::replace(&mut self.audio.effects, effects) != effects {
            self.with_backend(|backend| backend.set_audio_effects(effects));
        }
    }

    /// Draws `effect` in the video area for media without video, or nothing; seen at once.
    pub fn set_visualizer(&mut self, effect: Option<VisualEffect>) {
        if std::mem::replace(&mut self.audio.visualizer, effect) != effect {
            self.with_backend(|backend| backend.set_visualizer(effect));
        }
    }

    /// Sets what the player does on its own when a device goes or another app needs audio.
    pub fn set_audio_policy(&mut self, policy: AudioPolicy) {
        self.audio.policy = policy;
    }

    /// The backend reported the devices plugged in now.
    pub(super) fn devices_reported(&mut self, devices: Vec<AudioDevice>, cx: &mut Context<Self>) {
        let before = self.audio.devices.replace(devices);
        let after = self.audio_devices();
        let lost = before.as_deref().and_then(|b| hotplug::lost(b, after, &self.audio.active));
        let changed = before.as_deref() != Some(after);
        self.reroute();
        if let Some(device) = lost {
            tracing::info!("engine: audio device {:?} went away", device.name);
            if self.audio.policy.pause_on_device_loss
                && self.is_playing_for_user()
                && !self.just_started_sounding()
            {
                self.pause(cx);
            }
            cx.emit(PlayerEvent::AudioDeviceLost(device));
        }
        if changed {
            cx.emit(PlayerEvent::AudioDevicesChanged);
        }
    }

    /// Moves the start of playback `by` into the past, for tests of [`DEVICE_LOSS_GRACE`].
    #[cfg(test)]
    pub(crate) fn age_playback(&mut self, by: Duration) {
        self.audio.sounding_since =
            self.audio.sounding_since.and_then(|since| since.checked_sub(by));
    }

    /// Whether playback started or resumed less than [`DEVICE_LOSS_GRACE`] ago.
    fn just_started_sounding(&self) -> bool {
        let recent =
            self.audio.sounding_since.is_some_and(|since| since.elapsed() < DEVICE_LOSS_GRACE);
        if recent {
            tracing::info!("engine: a device went away as playback started; playing on");
        }
        recent
    }

    /// The OS asked for this app's audio to stop (`corked`) or allowed it again.
    pub(super) fn corked(&mut self, corked: bool, cx: &mut Context<Self>) {
        if corked && self.audio.policy.pause_when_corked && self.is_playing_for_user() {
            self.pause(cx);
            self.audio.paused_for_cork = self.intent == Intent::Pause;
        } else if !corked && std::mem::take(&mut self.audio.paused_for_cork) {
            self.play(cx);
        }
    }

    /// Sends the device to play through to the backend, if it changed.
    fn reroute(&mut self) {
        let route = hotplug::route(&self.audio.preferred, self.audio_devices());
        // Until the devices are known, trust the user's choice.
        let route = if self.audio.devices.is_none() { self.audio.preferred.clone() } else { route };
        if route != self.audio.active {
            self.with_backend(|backend| backend.set_audio_device(&route));
            self.audio.active = route;
        }
    }

    /// Whether media is playing, or about to, because the user asked for it.
    pub(super) fn is_playing_for_user(&self) -> bool {
        self.intent == Intent::Play
            && matches!(
                self.state,
                PlaybackState::Playing | PlaybackState::Buffering | PlaybackState::Opening
            )
    }
}
