//! OS accessibility preferences (S0.3.6, S9.1.4): "reduce motion", high contrast and the text
//! size.
//!
//! GPUI doesn't read these from the OS, so we do. Windows is queried directly
//! (cheap, synchronous). macOS and Linux shell out to `defaults` / `gsettings`, so
//! they run on a background thread. Neither OS reports high-contrast *colors*, so
//! [`SystemPrefs::high_contrast`] is only ever set on Windows.

use super::palette::SystemColors;

/// The OS accessibility preferences the theme follows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SystemPrefs {
    /// The OS asks for fewer animations.
    pub reduce_motion: bool,
    /// Windows High Contrast is on, with these colors.
    pub high_contrast: Option<SystemColors>,
    /// How much larger than designed text is drawn (`text_scale`), 1 for as designed.
    pub text_scale: f32,
}

impl Default for SystemPrefs {
    fn default() -> Self {
        SystemPrefs { reduce_motion: false, high_contrast: None, text_scale: 1. }
    }
}

/// Whether [`read`] is cheap enough to call on the UI thread.
pub const READ_IS_CHEAP: bool = cfg!(target_os = "windows");

/// Reads the current preferences. Blocks on a subprocess outside Windows, so call
/// it on a background thread unless [`READ_IS_CHEAP`].
pub fn read() -> SystemPrefs {
    let prefs = imp::read();
    match super::text_scale::debug_override() {
        Some(text_scale) => SystemPrefs { text_scale, ..prefs },
        None => prefs,
    }
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
        // macOS has no text size for all apps, only per-app sliders for Apple's own.
        SystemPrefs { reduce_motion, high_contrast: None, text_scale: 1. }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod imp {
    use super::{SystemPrefs, output};
    use crate::theme::text_scale;

    /// Reads the current preferences, blocking on a subprocess.
    pub fn read() -> SystemPrefs {
        let reduce_motion =
            output("gsettings", &["get", "org.gnome.desktop.interface", "enable-animations"])
                .is_some_and(|value| value == "false");
        let text_scale =
            output("gsettings", &["get", "org.gnome.desktop.interface", "text-scaling-factor"])
                .map_or(1., |value| text_scale::parse_gsettings(&value));
        SystemPrefs { reduce_motion, high_contrast: None, text_scale }
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
