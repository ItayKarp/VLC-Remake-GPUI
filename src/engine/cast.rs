//! Casting (S8.11): the cast devices found on the network, and how media plays on one.
//!
//! libVLC 3 finds Chromecasts over mDNS (Bonjour on macOS, microdns elsewhere) and plays to one
//! through its `chromecast` stream output: the device fetches the media from a small HTTP server
//! VLC runs, and what it can't play is converted first, as well as "Cast quality" says. Nothing
//! shows or sounds here meanwhile. That output carries sound and picture only, so subtitles
//! don't reach the device.

use crate::settings::CastQuality;

/// Names a cast device for as long as the backend knows it; a device that goes and comes back
/// gets a new one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CastDeviceId(pub u64);

/// A cast device found on the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CastDevice {
    /// Which device it is.
    pub id: CastDeviceId,
    /// The name its owner gave it, such as "Living Room TV".
    pub name: String,
    /// Whether it shows pictures; a speaker only plays sound.
    pub video: bool,
}

/// How the backend looks for cast devices and converts media for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CastSetup {
    /// "Discover cast devices": look for them on the network. Off, none are known, and media
    /// casting comes back here.
    pub discover: bool,
    /// "Cast quality", from the next cast on.
    pub quality: CastQuality,
}

impl Default for CastSetup {
    fn default() -> Self {
        Self { discover: false, quality: CastQuality::Automatic }
    }
}

/// libVLC's `sout-chromecast-conversion-quality`: 0 is High, 1 Medium, 2 Low (and 3 Low CPU).
/// libVLC 3 can't follow the network, so Automatic is its own desktop default, Medium.
pub fn conversion_quality(quality: CastQuality) -> i64 {
    match quality {
        CastQuality::High => 0,
        CastQuality::Automatic | CastQuality::Medium => 1,
        CastQuality::Low => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_follows_libvlcs_scale() {
        assert_eq!(conversion_quality(CastQuality::High), 0);
        assert_eq!(
            conversion_quality(CastQuality::Automatic),
            conversion_quality(CastQuality::Medium)
        );
        assert_eq!(conversion_quality(CastQuality::Low), 2);
    }
}
