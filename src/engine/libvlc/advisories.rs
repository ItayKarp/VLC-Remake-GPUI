//! The oldest libVLC without a published security problem (S9.7.1). libVLC reads every media
//! file and stream VLC plays, so its parsers are most of the attack surface; packages bundle
//! the release in `vendor/libvlc`, but the `.deb`, `.rpm` and a `RVLC_LIBVLC_DIR` build run
//! whatever libVLC the computer has. Raise [`SECURITY_FLOOR`] with each VideoLAN security
//! bulletin (<https://www.videolan.org/security/>), as `docs/security.md` describes.

/// The first release with every published VideoLAN security bulletin fixed: 3.0.23 (January
/// 2026) fixed issues left in 3.0.22, whose bulletin (December 2025) covered 3.0.21 and
/// earlier.
pub const SECURITY_FLOOR: Version = Version(3, 0, 23);

/// A libVLC release number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// The release in libVLC's version string (`3.0.23 Vetinari`, `3.0.21-2`); `None` when it
    /// doesn't start with a number.
    pub fn parse(text: &str) -> Option<Self> {
        let number = text.split([' ', '-', '+', '~']).next()?;
        let mut parts = number.split('.').map(str::parse::<u32>);
        let major = parts.next()?.ok()?;
        let minor = parts.next().and_then(Result::ok).unwrap_or(0);
        let patch = parts.next().and_then(Result::ok).unwrap_or(0);
        Some(Self(major, minor, patch))
    }
}

/// Logs a warning when `version` is older than [`SECURITY_FLOOR`] or can't be read.
pub fn warn_if_outdated(version: &str) {
    if let Some(warning) = outdated_warning(version) {
        tracing::warn!("libvlc: {warning}");
    }
}

/// What to say about `version`: `None` when it is at least [`SECURITY_FLOOR`].
fn outdated_warning(version: &str) -> Option<String> {
    let Some(found) = Version::parse(version) else {
        return Some(format!("can't tell how up to date version {version:?} is"));
    };
    let Version(major, minor, patch) = SECURITY_FLOOR;
    (found < SECURITY_FLOOR).then(|| {
        format!(
            "{version} has known security problems; update it to {major}.{minor}.{patch} or later"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_release_numbers() {
        assert_eq!(Version::parse("3.0.23 Vetinari"), Some(Version(3, 0, 23)));
        assert_eq!(Version::parse("3.0.21-2"), Some(Version(3, 0, 21)));
        assert_eq!(Version::parse("4.0"), Some(Version(4, 0, 0)));
        assert_eq!(Version::parse("Vetinari"), None);
    }

    #[test]
    fn only_releases_below_the_floor_are_warned_about() {
        assert_eq!(outdated_warning("3.0.23 Vetinari"), None);
        assert!(outdated_warning("3.0.16").is_some_and(|text| text.contains("3.0.23 or later")));
        assert!(outdated_warning("unknown").is_some());
    }

    #[test]
    fn older_releases_are_below_the_floor() {
        assert!(Version(3, 0, 22) < SECURITY_FLOOR);
        assert!(Version(3, 0, 16) < SECURITY_FLOOR);
        assert!(Version(3, 1, 0) > SECURITY_FLOOR);
        assert!(Version(4, 0, 0) > SECURITY_FLOOR);
    }

    #[test]
    fn the_flatpak_builds_a_libvlc_at_the_floor() {
        let manifest = include_str!("../../../packaging/linux/flatpak/org.redesignedvlc.app.yml");
        let source =
            manifest.split("videolan/vlc/").nth(1).expect("the manifest names VLC's source");
        let found = Version::parse(source.split('/').next().unwrap_or_default());
        assert!(found >= Some(SECURITY_FLOOR), "the Flatpak builds {found:?}");
    }
}
