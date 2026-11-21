//! "Copy Debug Info" (S0.7.4): what a bug report needs, without personal data. The
//! preferences hold no paths by design (see [`crate::settings`]); of the
//! machine-local state only the parts that aren't paths or window positions are
//! included.

use std::fmt::Write as _;

use super::system;
use crate::settings::Settings;

/// The text to copy: versions, OS, GPU and the settings without paths.
pub fn text(settings: &Settings, gpu: &str) -> String {
    let mut text = format!(
        "VLC {}\nOS: {}\nGPU: {gpu}\nlibVLC: {}\n",
        system::VERSION,
        system::os(),
        system::LIBVLC_VERSION,
    );
    let prefs = toml::to_string(&settings.prefs)
        .unwrap_or_else(|err| format!("# couldn't list the preferences: {err}\n"));
    let local = &settings.local;
    // Writing to a String can't fail.
    let _ = write!(
        text,
        "\n# Preferences\n{prefs}\n# This machine\nonboarding_complete = {}\nmedia_folders = {}\n",
        local.onboarding_complete,
        local.media_folders.len(),
    );
    text
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn lists_versions_and_settings_but_no_paths() {
        let mut settings = Settings::default();
        settings.local.media_folders = vec![PathBuf::from("/home/pat/Secret Videos")];
        let text = text(&settings, "Test GPU");
        assert!(text.starts_with(&format!("VLC {}\n", system::VERSION)), "{text}");
        assert!(text.contains("GPU: Test GPU"), "{text}");
        assert!(text.contains("libVLC: "), "{text}");
        assert!(text.contains("log_level = \"info\""), "{text}");
        assert!(text.contains("media_folders = 1"), "{text}");
        assert!(!text.contains("Secret"), "{text}");
    }
}
