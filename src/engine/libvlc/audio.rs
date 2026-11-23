//! Audio output on the worker thread (S1.5): reading the device list, choosing a device, and
//! the speaker setup. libVLC 3 sends no event when a device is plugged in or out, so the list
//! is read again every [`DEVICE_POLL`].

use std::ffi::{CString, c_int};
use std::time::{Duration, Instant};

use super::api::borrowed_string;
use super::worker::Worker;
use crate::engine::audio::{AudioConfig, AudioDevice, OutputDevice, SpeakerSetup};
use crate::engine::backend::BackendEvent;

/// How often the device list is read again.
pub const DEVICE_POLL: Duration = Duration::from_secs(2);
/// How long the list is left unread after the media is reopened where it is.
const SETTLE: Duration = Duration::from_millis(1500);

/// The id libVLC's output gives the OS default. CoreAudio lists it as "System Sound Output
/// Device" with id `0`, and follows the OS default while it is selected.
#[cfg(target_os = "macos")]
const SYSTEM_DEFAULT_ID: &str = "0";
/// The id libVLC's output gives the OS default: the empty id, which MMDevice and PulseAudio
/// take as "the default device".
#[cfg(not(target_os = "macos"))]
const SYSTEM_DEFAULT_ID: &str = "";

/// `libvlc_AudioChannel_Stereo`: downmix to two channels.
const CHANNEL_STEREO: c_int = 1;
/// libVLC's "unset" stereo mode: the output follows the device.
const CHANNEL_AUTOMATIC: c_int = 0;

/// Turns passthrough on for every output: a libVLC core option.
const PASSTHROUGH: &str = "--spdif";
/// Windows' MMDevice output has its own passthrough switch (1: AC3 and DTS).
#[cfg(windows)]
const MMDEVICE_PASSTHROUGH: &str = "--mmdevice-passthrough=1";

/// The libVLC instance arguments `config` adds.
pub fn arguments(config: AudioConfig) -> Vec<String> {
    let mut arguments = Vec::new();
    if config.passthrough {
        arguments.push(PASSTHROUGH.to_owned());
        #[cfg(windows)]
        arguments.push(MMDEVICE_PASSTHROUGH.to_owned());
    }
    arguments.extend(super::audio_effects::arguments(&config.effects));
    arguments
}

/// When the device list is next read, and what it held last time.
#[derive(Debug)]
pub struct DeviceWatch {
    known: Option<Vec<AudioDevice>>,
    next_poll: Instant,
    /// Until when the list isn't read ([`Self::settle`]).
    settled_at: Instant,
}

impl Default for DeviceWatch {
    /// A watch that reads the list at once.
    fn default() -> Self {
        let now = Instant::now();
        Self { known: None, next_poll: now, settled_at: now }
    }
}

impl DeviceWatch {
    /// How long until the list is due to be read, from `now`.
    pub fn until_poll(&self, now: Instant) -> Duration {
        self.next_poll.saturating_duration_since(now)
    }

    /// Leaves the list unread for a while from `now`: a reopen stops libVLC's audio output,
    /// which then lists no devices until a while after the new one starts, and that would read
    /// as the device going away.
    pub fn settle(&mut self, now: Instant) {
        self.settled_at = now + SETTLE;
        self.next_poll = self.next_poll.max(self.settled_at);
    }
}

impl Worker {
    /// Reads the device list if it is due.
    pub(super) fn poll_devices_if_due(&mut self) {
        if self.devices.until_poll(Instant::now()).is_zero() {
            self.poll_devices();
        }
    }

    /// Reads the device list now, and reports it if it changed; put off while it settles after
    /// a reopen ([`DeviceWatch::settle`]).
    pub(super) fn poll_devices(&mut self) {
        let now = Instant::now();
        if now < self.devices.settled_at {
            self.devices.next_poll = self.devices.settled_at;
            return;
        }
        self.devices.next_poll = Instant::now() + DEVICE_POLL;
        let devices = self.read_devices();
        if self.devices.known.as_ref() != Some(&devices) {
            self.devices.known = Some(devices.clone());
            self.shared.send(BackendEvent::AudioDevices(devices));
        }
    }

    /// Plays through `device` from now on.
    pub(super) fn set_audio_device(&self, device: &OutputDevice) {
        let id = match device {
            OutputDevice::SystemDefault => SYSTEM_DEFAULT_ID,
            OutputDevice::Device(id) => id,
        };
        let Ok(id) = CString::new(id) else { return };
        // SAFETY: the player is live; a NULL module means the current output, and `id` is
        // NUL-terminated for the call.
        unsafe {
            (self.vlc.api.libvlc_audio_output_device_set)(
                self.handles.player,
                std::ptr::null(),
                id.as_ptr(),
            );
        }
    }

    /// Mixes for `setup` from now on.
    pub(super) fn set_speaker_setup(&mut self, setup: SpeakerSetup) {
        self.speaker_setup = setup;
        self.apply_channels(setup);
    }

    /// Applies a forced speaker setup to newly started media. Automatic is libVLC's own
    /// behavior, and setting it would restart the output for nothing.
    pub(super) fn apply_speaker_setup(&self) {
        if self.speaker_setup != SpeakerSetup::Automatic {
            self.apply_channels(self.speaker_setup);
        }
    }

    fn apply_channels(&self, setup: SpeakerSetup) {
        let channel = match setup {
            SpeakerSetup::Automatic => CHANNEL_AUTOMATIC,
            SpeakerSetup::Stereo => CHANNEL_STEREO,
        };
        // SAFETY: the player is live.
        if unsafe { (self.vlc.api.libvlc_audio_set_channel)(self.handles.player, channel) } != 0 {
            // Without an output yet; applied again when the next media starts.
            tracing::debug!("libvlc: couldn't set the speaker setup now");
        }
    }

    /// The output's devices, without the system default.
    fn read_devices(&self) -> Vec<AudioDevice> {
        let api = &self.vlc.api;
        // SAFETY: the player is live; the list is released below.
        let list = unsafe { (api.libvlc_audio_output_device_enum)(self.handles.player) };
        let mut devices = Vec::new();
        let mut node = list;
        while !node.is_null() {
            // SAFETY: `node` is a live list entry until the list is released.
            let entry = unsafe { &*node };
            // SAFETY: libVLC's strings are NULL or NUL-terminated.
            let id = unsafe { borrowed_string(entry.device) }.unwrap_or_default();
            // SAFETY: as above.
            let name = unsafe { borrowed_string(entry.description) };
            if !id.is_empty() && id != SYSTEM_DEFAULT_ID {
                devices.push(AudioDevice { name: name.unwrap_or_else(|| id.clone()), id });
            }
            node = entry.next;
        }
        if !list.is_null() {
            // SAFETY: the list came from `device_enum` and is released once.
            unsafe { (api.libvlc_audio_output_device_list_release)(list) };
        }
        devices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_settling_list_is_read_once_it_has_settled() {
        let mut watch = DeviceWatch::default();
        let now = Instant::now();
        assert!(watch.until_poll(now).is_zero(), "read at once at first");
        watch.settle(now);
        assert_eq!(watch.until_poll(now), SETTLE);
    }
}
