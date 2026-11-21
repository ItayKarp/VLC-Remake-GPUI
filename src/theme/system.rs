//! OS accessibility preferences (S0.3.6): "reduce motion" and high contrast.
//!
//! GPUI doesn't read these from the OS, so we do. Windows is queried directly
//! (cheap, synchronous). macOS and Linux shell out to `defaults` / `gsettings`, so
//! they run on a background thread. Neither OS reports high-contrast *colors*, so
//! [`SystemPrefs::high_contrast`] is only ever set on Windows.

use super::palette::SystemColors;

/// The OS accessibility preferences the theme follows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SystemPrefs {
    /// The OS asks for fewer animations.
    pub reduce_motion: bool,
    /// Windows High Contrast is on, with these colors.
    pub high_contrast: Option<SystemColors>,
}

/// Whether [`read`] is cheap enough to call on the UI thread.
pub const READ_IS_CHEAP: bool = cfg!(target_os = "windows");

/// Reads the current preferences. Blocks on a subprocess outside Windows, so call
/// it on a background thread unless [`READ_IS_CHEAP`].
pub fn read() -> SystemPrefs {
    imp::read()
}

#[cfg(target_os = "windows")]
mod imp;

#[cfg(target_os = "macos")]
mod imp {
    use super::{SystemPrefs, output};

    /// Reads the current preferences, blocking on a subprocess.
    pub fn read() -> SystemPrefs {
        let reduce_motion =
            output("defaults", &["read", "com.apple.universalaccess", "reduceMotion"])
                .is_some_and(|value| value == "1");
        SystemPrefs { reduce_motion, high_contrast: None }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    use super::{SystemPrefs, output};

    /// Reads the current preferences, blocking on a subprocess.
    pub fn read() -> SystemPrefs {
        let reduce_motion =
            output("gsettings", &["get", "org.gnome.desktop.interface", "enable-animations"])
                .is_some_and(|value| value == "false");
        SystemPrefs { reduce_motion, high_contrast: None }
    }
}

/// A command's trimmed stdout, or `None` if it isn't installed or fails.
#[cfg(not(target_os = "windows"))]
fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
}
