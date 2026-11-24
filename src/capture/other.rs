//! Capture devices on Windows: DirectShow's default camera, and the screen. Listing cameras by
//! name takes COM's device enumeration, which the app doesn't build, so the default camera is
//! the one offered.

use super::{Access, Camera, Format, Screen};
use crate::app::media::Device;

/// `dshow` takes a size and frame rate.
pub const CAMERA_FORMATS: bool = true;

/// The default camera.
pub fn cameras() -> Vec<Camera> {
    vec![Camera { name: tr!("Default Camera").into(), id: String::new() }]
}

/// The screen, which libVLC captures whole.
pub fn screens() -> Vec<Screen> {
    vec![Screen { name: tr!("Screen").into(), id: None }]
}

/// Windows says whether a camera may be used only when it is opened.
pub fn camera_access() -> Access {
    Access::NotAsked
}

/// Windows lets any desktop app record the screen.
pub fn screen_access() -> Access {
    Access::Granted
}

/// Settings → Privacy & security → Camera; the screen has no page.
pub fn privacy_settings(screen: bool) -> Option<&'static str> {
    (!screen).then_some("ms-settings:privacy-webcam")
}

/// `dshow` on `camera` (the default one when its id is empty), at `format`.
pub fn camera(camera: &Camera, format: Format) -> Device {
    let mut device = Device::new("dshow://").expect("dshow:// is a device MRL");
    if !camera.id.is_empty() {
        device = device.with_option(format!(":dshow-vdev={}", camera.id));
    }
    if let Some(size) = format.size {
        device = device.with_option(format!(":dshow-size={}x{}", size.width, size.height));
    }
    match format.fps {
        Some(fps) => device.with_option(format!(":dshow-fps={fps}")),
        None => device,
    }
}
