//! Routing around devices that come and go (S1.5.4): which device to play through, and which
//! device that was playing has just disappeared.

use super::{AudioDevice, OutputDevice};

/// Where to play: the preferred device while it is plugged in, else the system default. A
/// preferred device that comes back is used again.
pub fn route(preferred: &OutputDevice, devices: &[AudioDevice]) -> OutputDevice {
    match preferred {
        OutputDevice::Device(id) if devices.iter().any(|device| &device.id == id) => {
            preferred.clone()
        }
        _ => OutputDevice::SystemDefault,
    }
}

/// The device sound was going to that is in `before` but not in `after`. Through a chosen
/// device that is that device. Through the system default it is any device that went away:
/// the backend doesn't say which device the OS default is, and the device that disappears
/// while something plays is almost always the one it played through (headphones).
pub fn lost(
    before: &[AudioDevice],
    after: &[AudioDevice],
    active: &OutputDevice,
) -> Option<AudioDevice> {
    let gone = |device: &&AudioDevice| !after.iter().any(|other| other.id == device.id);
    match active {
        OutputDevice::Device(id) => before.iter().filter(gone).find(|device| &device.id == id),
        OutputDevice::SystemDefault => before.iter().find(gone),
    }
    .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str) -> AudioDevice {
        AudioDevice { id: id.into(), name: format!("Device {id}") }
    }

    #[test]
    fn a_missing_preferred_device_falls_back_to_the_default_and_returns() {
        let headphones = OutputDevice::Device("2".into());
        assert_eq!(route(&headphones, &[device("1")]), OutputDevice::SystemDefault);
        assert_eq!(route(&headphones, &[device("1"), device("2")]), headphones);
        assert_eq!(
            route(&OutputDevice::SystemDefault, &[device("1")]),
            OutputDevice::SystemDefault
        );
    }

    #[test]
    fn only_the_chosen_device_counts_as_lost() {
        let before = [device("1"), device("2")];
        let chosen = OutputDevice::Device("1".into());
        assert_eq!(lost(&before, &[device("1")], &chosen), None);
        assert_eq!(lost(&before, &[device("2")], &chosen), Some(device("1")));
    }

    #[test]
    fn through_the_default_any_removed_device_counts() {
        let before = [device("1"), device("2")];
        let default = OutputDevice::SystemDefault;
        assert_eq!(lost(&before, &[device("1")], &default), Some(device("2")));
        assert_eq!(lost(&before, &before, &default), None);
        assert_eq!(lost(&[device("1")], &before, &default), None);
    }
}
