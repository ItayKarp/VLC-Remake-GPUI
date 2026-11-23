//! Audio output (S1.5): the devices a backend can play through, which one the user wants, how
//! speakers are set up, and how the player routes around a device that disappears.

pub mod effects;
pub mod equalizer;
pub mod hotplug;
pub mod test_tone;
pub mod visualizer;

use serde::{Deserialize, Serialize};

/// An output device the backend reported, other than the system default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDevice {
    /// The backend's id for it, stable while the device stays plugged in.
    pub id: String,
    /// Its name, for the user ("AirPods Pro", "MacBook Air Speakers").
    pub name: String,
}

/// Where the user wants sound to go.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum OutputDevice {
    /// The OS's default output, following it when the user changes it in the OS.
    #[default]
    SystemDefault,
    /// One device, by its [`AudioDevice::id`].
    Device(String),
}

impl OutputDevice {
    /// The device a stored id names; an empty id is the system default.
    pub fn from_id(id: &str) -> Self {
        if id.is_empty() { Self::SystemDefault } else { Self::Device(id.to_owned()) }
    }

    /// The id to store for this choice; empty for the system default.
    pub fn id(&self) -> &str {
        match self {
            Self::SystemDefault => "",
            Self::Device(id) => id,
        }
    }
}

/// How multichannel audio meets the speakers ("Speaker setup" in Settings → Audio).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerSetup {
    /// Match the device: 5.1 and 7.1 are downmixed for a stereo device, kept for a
    /// multichannel one.
    #[default]
    Automatic,
    /// Always downmix to two channels.
    Stereo,
}

/// How the two channels play ("Stereo Mode", S7.6.2): as they are, or remixed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StereoMode {
    /// As the media has them; the speaker setup decides how surround is mixed.
    #[default]
    Stereo,
    /// Both channels mixed into one, on both speakers.
    Mono,
    /// The left channel on both speakers.
    Left,
    /// The right channel on both speakers.
    Right,
    /// Left and right swapped.
    ReverseStereo,
    /// Dolby Surround encoded in the two channels, decoded.
    DolbySurround,
    /// Surround mixed for headphones, as if heard from speakers around the listener.
    Headphones,
}

/// Audio choices a backend makes once, when it is created. Changes apply to the next backend
/// (the next launch).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AudioConfig {
    /// Send AC3 and DTS undecoded to the receiver (HDMI, S/PDIF) where the device takes them.
    pub passthrough: bool,
}

/// What the player does on its own when the audio situation changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioPolicy {
    /// Pause when the device that was playing disappears (headphones unplugged).
    pub pause_on_device_loss: bool,
    /// Pause while the OS says another app needs the audio (a call), and resume after.
    pub pause_when_corked: bool,
}

impl Default for AudioPolicy {
    /// Both on, as VLC does.
    fn default() -> Self {
        Self { pause_on_device_loss: true, pause_when_corked: true }
    }
}
