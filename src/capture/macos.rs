//! Capture devices on macOS: cameras from AVFoundation, displays from CoreGraphics, and the
//! Camera and Screen Recording permissions (TCC).

use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeVideo};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayIsBuiltin, CGDisplayPixelsHigh, CGDisplayPixelsWide,
    CGGetActiveDisplayList, CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess,
};

use super::{Access, Camera, Format, Screen};
use crate::app::media::Device;

/// libVLC 3's `avcapture` takes no format.
pub const CAMERA_FORMATS: bool = false;
/// The most displays listed.
const MAX_DISPLAYS: usize = 16;

/// The cameras AVFoundation lists, the default first.
pub fn cameras() -> Vec<Camera> {
    // SAFETY: `AVMediaTypeVideo` is a constant AVFoundation defines.
    let Some(video) = (unsafe { AVMediaTypeVideo }) else { return Vec::new() };
    // SAFETY: plain AVFoundation queries, safe from any thread.
    #[allow(deprecated, reason = "lists the same cameras as a discovery session, in one call")]
    let (devices, default) = unsafe {
        (
            AVCaptureDevice::devicesWithMediaType(video),
            AVCaptureDevice::defaultDeviceWithMediaType(video),
        )
    };
    // SAFETY: as above.
    let default_id = default.map(|device| unsafe { device.uniqueID() }.to_string());
    let mut cameras: Vec<Camera> = devices
        .iter()
        .map(|device| Camera {
            // SAFETY: as above.
            name: unsafe { device.localizedName() }.to_string(),
            // SAFETY: as above.
            id: unsafe { device.uniqueID() }.to_string(),
        })
        .collect();
    cameras.sort_by_key(|camera| Some(&camera.id) != default_id.as_ref());
    cameras
}

/// The active displays, the main one first.
pub fn screens() -> Vec<Screen> {
    let mut ids = [CGDirectDisplayID::default(); MAX_DISPLAYS];
    let mut count = 0;
    // SAFETY: `ids` holds `MAX_DISPLAYS` entries and `count` receives how many were filled.
    let listed =
        unsafe { CGGetActiveDisplayList(MAX_DISPLAYS as u32, ids.as_mut_ptr(), &raw mut count) };
    if listed.0 != 0 {
        return Vec::new();
    }
    let ids = &ids[..(count as usize).min(MAX_DISPLAYS)];
    ids.iter().enumerate().map(|(index, &id)| screen(id, index)).collect()
}

/// The display `id`, the `index`th listed.
fn screen(id: CGDirectDisplayID, index: usize) -> Screen {
    let size = format!("{} × {}", CGDisplayPixelsWide(id), CGDisplayPixelsHigh(id));
    let name = if CGDisplayIsBuiltin(id) {
        tr!("Built-in Display ({size})", size)
    } else {
        tr!("Display {number} ({size})", number = index + 1, size)
    };
    Screen { name, id: Some(id) }
}

/// Whether the Camera permission allows this app.
pub fn camera_access() -> Access {
    // SAFETY: `AVMediaTypeVideo` is a constant AVFoundation defines.
    let Some(video) = (unsafe { AVMediaTypeVideo }) else { return Access::Denied };
    // SAFETY: a plain AVFoundation query for a media type it accepts.
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(video) } {
        AVAuthorizationStatus::Authorized => Access::Granted,
        AVAuthorizationStatus::NotDetermined => Access::NotAsked,
        _ => Access::Denied,
    }
}

/// Whether the Screen Recording permission allows this app. The first time, macOS asks, and
/// the answer counts from the app's next launch.
pub fn screen_access() -> Access {
    if CGPreflightScreenCaptureAccess() {
        return Access::Granted;
    }
    if CGRequestScreenCaptureAccess() { Access::Granted } else { Access::Denied }
}

/// System Settings → Privacy & Security, on Screen Recording or Camera.
#[allow(clippy::unnecessary_wraps, reason = "the other platforms' have no page for some kinds")]
pub fn privacy_settings(screen: bool) -> Option<&'static str> {
    Some(if screen {
        "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
    } else {
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Camera"
    })
}

/// `avcapture` on `camera`; it picks its own format.
pub fn camera(camera: &Camera, _: Format) -> Device {
    Device::new(&format!("avcapture://{}", camera.id)).expect("avcapture:// is a device MRL")
}
