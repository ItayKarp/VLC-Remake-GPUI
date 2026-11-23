//! Capture devices (S8.9.2): the cameras and capture cards plugged in, the screens, what each
//! opens as in libVLC, and whether the OS lets this app use them.
//!
//! libVLC captures cameras with `avcapture` on macOS, `v4l2` on Linux and `dshow` on Windows,
//! and screens with `screen` everywhere. macOS's `avcapture` in libVLC 3 takes no format: the
//! camera picks its own, so only Linux and Windows offer a size and frame rate for cameras.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod other;

#[cfg(target_os = "linux")]
use self::linux as platform;
#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use self::other as platform;
use crate::app::media::{Device, Media};

/// A camera or capture card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Camera {
    /// Its name, as the OS gives it.
    pub name: String,
    /// What libVLC opens it by: AVFoundation's unique id, a `/dev/video` path, or a DirectShow
    /// name. Empty for the system's default camera.
    pub id: String,
}

/// A screen to capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// Its name: "Built-in Display", or "Display" and its number, with its size.
    pub name: String,
    /// What libVLC's `screen` module picks it by (its CoreGraphics display id on macOS); `None`
    /// for the main one.
    pub id: Option<u32>,
}

/// What to capture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A camera or capture card.
    Camera(Camera),
    /// A screen.
    Screen(Screen),
}

/// A picture size, in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// The format to capture in; `None` leaves each to the device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Format {
    /// The picture size; cameras only.
    pub size: Option<Size>,
    /// Frames per second.
    pub fps: Option<u32>,
}

/// The sizes offered for cameras: the common 4:3 and 16:9 ones.
pub const SIZES: [Size; 4] = [
    Size { width: 640, height: 480 },
    Size { width: 1280, height: 720 },
    Size { width: 1920, height: 1080 },
    Size { width: 3840, height: 2160 },
];
/// The frame rates offered.
pub const FRAME_RATES: [u32; 5] = [5, 15, 24, 30, 60];
/// The frame rate a screen is captured at when none is picked: libVLC's own 1 a second is too
/// slow to watch.
const SCREEN_FPS: u32 = 30;

/// Whether the OS lets this app use a camera or record the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// It may.
    Granted,
    /// It hasn't been asked; opening the device asks.
    NotAsked,
    /// The user or a policy refused; only the OS's privacy settings can change it.
    Denied,
}

/// The cameras and capture cards plugged in, the default first. Blocks briefly.
pub fn cameras() -> Vec<Camera> {
    platform::cameras()
}

/// The screens, the main one first.
pub fn screens() -> Vec<Screen> {
    platform::screens()
}

/// Whether a camera's size and frame rate can be picked here.
pub const CAMERA_FORMATS: bool = platform::CAMERA_FORMATS;

/// Whether this app may use the camera.
pub fn camera_access() -> Access {
    platform::camera_access()
}

/// Whether this app may record the screen; asks the first time, where the OS asks.
pub fn screen_access() -> Access {
    platform::screen_access()
}

/// The OS's privacy settings page for `source`'s kind of device, if it has one.
pub fn privacy_settings(source: &Source) -> Option<&'static str> {
    platform::privacy_settings(matches!(source, Source::Screen(_)))
}

/// The media that captures `source` in `format`, named after it.
pub fn media(source: &Source, format: Format) -> Media {
    let device = match source {
        Source::Camera(camera) => platform::camera(camera, format),
        Source::Screen(screen) => screen_device(screen, format),
    };
    let name = match source {
        Source::Camera(camera) => &camera.name,
        Source::Screen(screen) => &screen.name,
    };
    Media::Device(device.titled(name))
}

/// libVLC's `screen` module, on `screen` at `format`'s frame rate.
fn screen_device(screen: &Screen, format: Format) -> Device {
    let device = Device::new("screen://").expect("screen:// is a device MRL");
    let fps = format.fps.unwrap_or(SCREEN_FPS);
    let device = device.with_option(format!(":screen-fps={fps}"));
    match screen.id {
        Some(id) => device.with_option(format!(":screen-display-id={id}")),
        None => device,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screens_capture_at_a_watchable_rate_by_display() {
        let screen = Screen { name: "Display 2".into(), id: Some(5) };
        let Media::Device(device) = media(&Source::Screen(screen), Format::default()) else {
            panic!("a device")
        };
        assert_eq!(device.mrl(), "screen://");
        assert!(device.options().contains(&":screen-fps=30".to_owned()));
        assert!(device.options().contains(&":screen-display-id=5".to_owned()));
        assert_eq!(device.display_name(), "Display 2");
    }
}
