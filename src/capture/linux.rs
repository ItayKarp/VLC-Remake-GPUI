//! Capture devices on Linux: Video4Linux cameras from `/sys/class/video4linux`, and the X
//! screen. Linux asks no permission of its own; a device the user can't open fails to play.

use std::fs;
use std::path::Path;

use super::{Access, Camera, Format, Screen};
use crate::app::media::Device;

/// `v4l2` takes a size and frame rate.
pub const CAMERA_FORMATS: bool = true;
/// Where the kernel lists Video4Linux devices.
const V4L_CLASS: &str = "/sys/class/video4linux";

/// The capture nodes listed, in name order. A camera has a metadata node beside its capture
/// node; only the first node of a device (index 0) captures pictures.
pub fn cameras() -> Vec<Camera> {
    let Ok(entries) = fs::read_dir(V4L_CLASS) else { return Vec::new() };
    let mut cameras: Vec<Camera> = entries
        .flatten()
        .filter(|entry| read(&entry.path().join("index")).is_none_or(|index| index == "0"))
        .map(|entry| {
            let node = entry.file_name().to_string_lossy().into_owned();
            let name = read(&entry.path().join("name")).unwrap_or_else(|| node.clone());
            Camera { name, id: format!("/dev/{node}") }
        })
        .collect();
    cameras.sort_by(|a, b| a.id.cmp(&b.id));
    cameras
}

/// The X screen, which libVLC captures whole.
pub fn screens() -> Vec<Screen> {
    vec![Screen { name: "Screen".into(), id: None }]
}

/// Linux has no camera permission.
pub fn camera_access() -> Access {
    Access::Granted
}

/// Linux has no screen recording permission.
pub fn screen_access() -> Access {
    Access::Granted
}

/// Linux has no privacy settings page to point to.
pub fn privacy_settings(_: bool) -> Option<&'static str> {
    None
}

/// `v4l2` on `camera`, at `format`.
pub fn camera(camera: &Camera, format: Format) -> Device {
    let mut device =
        Device::new(&format!("v4l2://{}", camera.id)).expect("v4l2:// is a device MRL");
    if let Some(size) = format.size {
        device = device
            .with_option(format!(":v4l2-width={}", size.width))
            .with_option(format!(":v4l2-height={}", size.height));
    }
    match format.fps {
        Some(fps) => device.with_option(format!(":v4l2-fps={fps}")),
        None => device,
    }
}

/// The trimmed text of the file at `path`.
fn read(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|text| text.trim().to_owned())
}
