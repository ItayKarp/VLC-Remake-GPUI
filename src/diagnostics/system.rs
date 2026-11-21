//! What the app runs on: its version, the OS and the GPU, for crash reports and
//! "Copy Debug Info".

use std::sync::OnceLock;

use gpui::Window;

/// The app's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The engine's version, such as "3.0.23 Vetinari", or why there is none.
/// Loads libVLC if nothing has yet.
pub fn libvlc_version() -> String {
    crate::engine::libvlc_version().unwrap_or_else(|| "unavailable".to_owned())
}

/// The engine's version if it is loaded, without loading it (a crash report can't).
pub fn loaded_libvlc_version() -> String {
    crate::engine::loaded_libvlc_version().unwrap_or_else(|| "not loaded".to_owned())
}

/// The GPU the first window renders with; GPUI only knows it once a window exists.
static GPU: OnceLock<String> = OnceLock::new();

/// The OS name, version and architecture, e.g. "Windows 11 Pro (26200) [64-bit]".
pub fn os() -> String {
    format!("{} ({})", os_info::get(), std::env::consts::ARCH)
}

/// Remembers the GPU `window` renders with, if no window has reported one yet.
pub fn remember_gpu(window: &Window) {
    if GPU.get().is_some() {
        return;
    }
    if let Some(specs) = window.gpu_specs() {
        let software = if specs.is_software_emulated { ", software" } else { "" };
        let description = format!(
            "{} ({} {}{software})",
            specs.device_name, specs.driver_name, specs.driver_info
        );
        GPU.set(description).ok();
    }
}

/// The GPU [`remember_gpu`] saw, or "unknown" before any window opened.
pub fn gpu() -> &'static str {
    GPU.get().map_or("unknown", String::as_str)
}
