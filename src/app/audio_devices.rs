//! The output devices the player last reported, for Settings → Audio's "Output device" (S5.8.2):
//! the player lives in the main window, and the Settings window lists what it found.
//!
//! The list is only known once a player exists (the first media opened). Until then the dropdown
//! offers the system default and the saved device, as the device list can't be read without
//! loading libVLC.

use gpui::{App, Global};

use crate::engine::audio::AudioDevice;
use crate::settings::registry::MachineChoice;

/// The devices plugged in, without the system default; `None` until a player reports them.
#[derive(Default)]
pub struct AudioDevices(Option<Vec<AudioDevice>>);

impl Global for AudioDevices {}

/// Remembers the devices a player reported.
pub fn set(devices: &[AudioDevice], cx: &mut App) {
    if cx.try_global::<AudioDevices>().and_then(|known| known.0.as_deref()) != Some(devices) {
        cx.set_global(AudioDevices(Some(devices.to_vec())));
    }
}

/// "Output device"'s options: the system default, each device, and the saved one when it isn't
/// plugged in (or the list isn't known yet).
pub fn choices(cx: &App, saved: &str) -> Vec<MachineChoice> {
    let known = cx.try_global::<AudioDevices>().and_then(|known| known.0.as_deref());
    options(known, saved)
}

fn options(known: Option<&[AudioDevice]>, saved: &str) -> Vec<MachineChoice> {
    let default = MachineChoice { id: String::new(), label: "System default".into(), glyph: None };
    let devices = known.unwrap_or_default().iter().map(|device| MachineChoice {
        id: device.id.clone(),
        label: device.name.clone(),
        glyph: None,
    });
    let mut choices: Vec<_> = std::iter::once(default).chain(devices).collect();
    if !choices.iter().any(|choice| choice.id == saved) {
        let label = if known.is_some() { "Unplugged device" } else { "Saved device" };
        choices.push(MachineChoice { id: saved.into(), label: label.into(), glyph: None });
    }
    choices
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str) -> AudioDevice {
        AudioDevice { id: id.into(), name: name.into() }
    }

    fn labels(choices: &[MachineChoice]) -> Vec<&str> {
        choices.iter().map(|choice| choice.label.as_str()).collect()
    }

    #[test]
    fn the_default_comes_first_then_each_device() {
        let devices = [device("a", "AirPods"), device("b", "Speakers")];
        assert_eq!(
            labels(&options(Some(&devices), "b")),
            ["System default", "AirPods", "Speakers"]
        );
    }

    #[test]
    fn a_saved_device_that_is_missing_stays_listed() {
        let devices = [device("a", "AirPods")];
        assert_eq!(labels(&options(Some(&devices), "z")).last(), Some(&"Unplugged device"));
        assert_eq!(labels(&options(None, "z")), ["System default", "Saved device"]);
        assert_eq!(labels(&options(None, "")), ["System default"]);
    }
}
