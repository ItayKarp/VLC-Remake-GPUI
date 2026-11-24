//! Why a disc or capture device couldn't play (S8.9's edge cases), in words about discs and
//! devices rather than files. libVLC 3 says only that opening failed, so each reason names what
//! it may be: an ejected or damaged disc, a DVD from another region or copy-protected, an
//! encrypted Blu-ray without the user's own AACS keys, a camera another app holds or the OS
//! doesn't let the app use. A capture device's failure comes with a button to the OS's privacy
//! settings, where it has them.

use gpui::Context;

use super::MainWindow;
use crate::app::media::{Device, DeviceKind, Media};
use crate::capture::{self, Camera, Screen, Source};
use crate::engine::state::ErrorKind;
use crate::ui::toast::{Level, Toast};

/// Why `device` couldn't play, after "Couldn't play …: ".
pub(super) fn reason(device: &Device, kind: ErrorKind) -> &'static str {
    match (device.kind(), kind) {
        (_, ErrorKind::NoPlayableStreams) if device.is_disc() => {
            tr!("the disc has nothing VLC can play.")
        }
        (_, ErrorKind::SourceLost) if device.is_disc() => {
            tr!("the disc was ejected, or a damaged part of it can't be read.")
        }
        (DeviceKind::Dvd, _) => {
            tr!("the DVD can't be read. It may be damaged, made for another region, or \
                 protected in a way VLC can't read.")
        }
        (DeviceKind::BluRay, _) => {
            tr!("the Blu-ray can't be read. An encrypted one plays only with libaacs and \
                 decryption keys installed by you; VLC includes neither.")
        }
        (DeviceKind::AudioCd | DeviceKind::Vcd, _) => {
            tr!("the disc can't be read. It may be damaged.")
        }
        (DeviceKind::Camera, ErrorKind::SourceLost) => tr!("the camera was unplugged or stopped."),
        (DeviceKind::Camera, _) => {
            tr!("the camera can't be opened. Another app may be using it, or VLC isn't allowed to.")
        }

        (DeviceKind::Screen, _) => tr!("the screen can't be recorded. VLC may not be allowed to."),
    }
}

impl MainWindow {
    /// Says why the disc or device playing failed, offering a capture device's privacy
    /// settings; files and streams are left to the status line.
    pub(super) fn explain_device_failure(&mut self, kind: ErrorKind, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let Some(device) = player.read(cx).media().and_then(Media::as_device).cloned() else {
            return;
        };
        let (name, reason) = (device.display_name(), reason(&device, kind));
        let text = tr!("Couldn't play {name}: {reason}", name, reason);
        let toast = Toast::new(Level::Error, text);
        let toast = match settings_page(&device) {
            Some(url) => toast.action(tr!("Open Privacy Settings"), move |_, cx| cx.open_url(url)),
            None => toast,
        };
        self.show_toast(toast, cx);
    }
}

/// The OS's privacy settings page for a capture device, if it has one.
fn settings_page(device: &Device) -> Option<&'static str> {
    let source = match device.kind() {
        DeviceKind::Camera => Source::Camera(Camera { name: String::new(), id: String::new() }),
        DeviceKind::Screen => Source::Screen(Screen { name: String::new(), id: None }),
        _ => return None,
    };
    capture::privacy_settings(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(mrl: &str) -> Device {
        Device::new(mrl).expect("a device MRL")
    }

    #[test]
    fn discs_and_devices_fail_in_their_own_words() {
        let dvd = device("dvd:///dev/rdisk4");
        assert!(reason(&dvd, ErrorKind::OpenFailed).contains("another region"));
        assert!(reason(&dvd, ErrorKind::SourceLost).contains("ejected"));
        assert!(reason(&device("bluray:///Volumes/X"), ErrorKind::OpenFailed).contains("libaacs"));
        let camera = device("avcapture://0x1");
        assert!(reason(&camera, ErrorKind::OpenFailed).contains("Another app"));
        assert!(settings_page(&camera).is_some() || cfg!(target_os = "linux"));
        assert_eq!(settings_page(&dvd), None);
    }
}
