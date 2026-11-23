//! The main window's side of audio (S1.5): it sets a new player up from the audio settings
//! (effects at its creation, S5.8.2), follows changes to them and to the settings panel's
//! session values over them (the equalizer and stereo mode at once, S7.6.2), remembers the
//! volume, and tells the user when the device that played disappears or there is no device at
//! all.

use gpui::{Context, Entity, Subscription, WeakEntity};

use super::MainWindow;
use crate::app::audio_devices;
use crate::effects::{self, EffectSession};
use crate::engine::audio::effects::AudioEffects;
use crate::engine::audio::equalizer::{Equalizer, GAIN_RANGE};
use crate::engine::audio::{AudioConfig, AudioPolicy, OutputDevice};
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::values::{MAX_VOLUME, MIN_MAX_VOLUME, Volume};
use crate::settings::{self, AudioSettings, Settings, SettingsStore, StartupVolume};
use crate::ui::toast::{Level, Toast};

/// The lowest and highest "Volume step", in percent.
const VOLUME_STEP_RANGE: (u32, u32) = (1, 100);

/// What the window keeps about audio.
#[derive(Default)]
pub(super) struct AudioWatch {
    /// Re-applies the audio settings to the player when they or the session change.
    settings: Option<[Subscription; 2]>,
    /// Whether the user was told there is no audio device, so it is said once per absence.
    warned_no_device: bool,
}

impl MainWindow {
    /// The backend's audio setup, from the settings.
    pub(super) fn audio_config(cx: &Context<Self>) -> AudioConfig {
        let audio = &settings::get(cx).prefs.audio;
        AudioConfig { passthrough: audio.passthrough, effects: effects(audio) }
    }

    /// Sets a new `player` up from the settings in effect, at the startup volume, and follows
    /// later changes to them.
    pub(super) fn set_up_audio(&mut self, player: &Entity<Player>, cx: &mut Context<Self>) {
        let settings = effects::effective(cx);
        player.update(cx, |player, cx| {
            apply(&settings, player, cx);
            player.set_volume(startup_volume(&settings), cx);
        });
        let follow = |player: WeakEntity<Player>| {
            move |_: &mut Self, cx: &mut Context<Self>| {
                let settings = effects::effective(cx);
                // The player lives as long as the window that owns this subscription.
                let _ = player.update(cx, |player, cx| apply(&settings, player, cx));
            }
        };
        let player = player.downgrade();
        self.audio.settings = Some([
            cx.observe_global::<SettingsStore>(follow(player.clone())),
            cx.observe_global::<EffectSession>(follow(player)),
        ]);
    }

    /// Handles the player's audio events.
    pub(super) fn on_audio_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        let Some((player, _)) = &self.player else { return };
        match event {
            PlayerEvent::VolumeChanged => {
                let percent = player.read(cx).volume().as_percent();
                settings::update(cx, |s| s.local.audio.last_volume_percent = percent);
            }
            PlayerEvent::AudioDeviceLost(device) => {
                let text =
                    format!("{} disconnected. Sound moved to the system default.", device.name);
                self.show_toast(Toast::new(Level::Info, text), cx);
            }
            PlayerEvent::AudioDevicesChanged => {
                let devices = player.read(cx).audio_devices().to_vec();
                audio_devices::set(&devices, cx);
                let none = player.read(cx).has_no_audio_device();
                if none && !self.audio.warned_no_device {
                    let text = "No audio device: media plays without sound.";
                    self.show_toast(Toast::new(Level::Warning, text), cx);
                }
                self.audio.warned_no_device = none;
            }
            _ => {}
        }
    }
}

/// Applies the settings that may change while playing.
fn apply(settings: &Settings, player: &mut Player, cx: &mut Context<Player>) {
    let audio = &settings.prefs.audio;
    player.set_max_volume(max_volume(settings), cx);
    player.set_audio_device(OutputDevice::from_id(&settings.local.audio.output_device), cx);
    player.set_speaker_setup(audio.speaker_setup, cx);
    player.set_stereo_mode(audio.stereo_mode, cx);
    player.set_equalizer(audio.equalizer.then(|| equalizer(audio)));
    player.set_audio_policy(AudioPolicy {
        pause_on_device_loss: audio.pause_on_device_disconnect,
        pause_when_corked: settings.prefs.player.pause_when_another_app_uses_audio,
    });
}

/// The loudness filters and effects the backend starts with.
fn effects(audio: &AudioSettings) -> AudioEffects {
    AudioEffects {
        normalize: audio.normalize_loudness.then_some(audio.loudness_target),
        night_mode: audio.night_mode,
        replay_gain: audio.replay_gain,
        replay_gain_preamp: audio.replay_gain_preamp,
        equalizer_two_pass: audio.equalizer_two_pass,
        spatial: audio.spatial_audio.then_some(audio.spatial_mode),
        stereo_widening: audio.stereo_widening,
    }
}

/// The equalizer's gains, each kept within libVLC's range.
fn equalizer(audio: &AudioSettings) -> Equalizer {
    let (low, high) = GAIN_RANGE;
    let gains = audio.equalizer_gains();
    Equalizer {
        preamp: gains.preamp.clamp(low, high),
        bands: gains.bands.map(|band| band.clamp(low, high)),
    }
}

/// "Maximum volume", kept within 100% to 300%.
fn max_volume(settings: &Settings) -> Volume {
    let percent = settings.prefs.audio.max_volume_percent;
    let range = MIN_MAX_VOLUME.as_percent()..=Volume::new(MAX_VOLUME).as_percent();
    Volume::from_percent(percent.clamp(*range.start(), *range.end()))
}

/// The volume a launch starts at: the last one, at most 100% so a boost doesn't carry over
/// (D21), or the fixed one; never above the maximum.
fn startup_volume(settings: &Settings) -> Volume {
    let audio = &settings.prefs.audio;
    let volume = match audio.startup_volume {
        StartupVolume::RememberLast => {
            Volume::from_percent(settings.local.audio.last_volume_percent).min(Volume::FULL)
        }
        StartupVolume::Fixed => Volume::from_percent(audio.startup_volume_percent),
    };
    volume.min(max_volume(settings))
}

/// "Volume step", kept within 1% to 100%.
pub(super) fn volume_step(settings: &Settings) -> Volume {
    let (low, high) = VOLUME_STEP_RANGE;
    Volume::from_percent(settings.prefs.audio.volume_step_percent.clamp(low, high))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(edit: impl FnOnce(&mut Settings)) -> Settings {
        let mut settings = Settings::default();
        edit(&mut settings);
        settings
    }

    #[test]
    fn a_remembered_boost_restarts_at_100_percent() {
        let loud = settings(|s| s.local.audio.last_volume_percent = 180);
        assert_eq!(startup_volume(&loud), Volume::FULL);
        let quiet = settings(|s| s.local.audio.last_volume_percent = 40);
        assert_eq!(startup_volume(&quiet), Volume::from_percent(40));
    }

    #[test]
    fn a_fixed_startup_volume_stays_under_the_maximum() {
        let fixed = settings(|s| {
            s.prefs.audio.startup_volume = StartupVolume::Fixed;
            s.prefs.audio.startup_volume_percent = 250;
        });
        assert_eq!(startup_volume(&fixed), Volume::from_percent(125));
    }

    #[test]
    fn out_of_range_settings_are_clamped() {
        let wild = settings(|s| {
            s.prefs.audio.max_volume_percent = 999;
            s.prefs.audio.volume_step_percent = 0;
        });
        assert_eq!(max_volume(&wild), Volume::from_percent(300));
        assert_eq!(volume_step(&wild), Volume::from_percent(1));
        let low = settings(|s| s.prefs.audio.max_volume_percent = 10);
        assert_eq!(max_volume(&low), Volume::FULL);
    }
}
