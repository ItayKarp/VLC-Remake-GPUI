//! Media that is neither a file nor a network stream (S8.9): a disc (DVD, Blu-ray, Audio CD,
//! VCD) or a capture device (a camera or capture card, or a screen), as libVLC opens it: an MRL
//! such as `dvd:///dev/rdisk4` or `screen://`, and the options it opens with
//! (`:cdda-track=3`, `:screen-fps=30`).
//!
//! The stored form (settings, the session, Open Recent) is the MRL followed by each option
//! after a space, with spaces and `%` in an option escaped, so a name such as
//! `:meta-title=Track One` survives. An MRL is a URL, so it has no spaces of its own.

/// What kind of device the media is on, read from its MRL's scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// A DVD, with or without its menus.
    Dvd,
    /// A Blu-ray.
    BluRay,
    /// An audio CD.
    AudioCd,
    /// A Video CD.
    Vcd,
    /// A camera or capture card.
    Camera,
    /// The screen.
    Screen,
}

/// The MRL schemes that open a device, and the kind each is.
const SCHEMES: &[(&str, DeviceKind)] = &[
    ("dvd", DeviceKind::Dvd),
    ("dvdnav", DeviceKind::Dvd),
    ("dvdsimple", DeviceKind::Dvd),
    ("bluray", DeviceKind::BluRay),
    ("cdda", DeviceKind::AudioCd),
    ("vcd", DeviceKind::Vcd),
    ("avcapture", DeviceKind::Camera),
    ("v4l2", DeviceKind::Camera),
    ("dshow", DeviceKind::Camera),
    ("screen", DeviceKind::Screen),
];

/// The option that names the media for the title bar and lists, as libVLC reads it too.
const TITLE_OPTION: &str = ":meta-title=";

/// The options VLC itself opens devices with; stored text (a session file, a saved playlist,
/// another VLC process) may carry no other, so it can't slip in `:sout` to write files or any
/// other libVLC option (S9.7.2).
const STORED_OPTIONS: &[&str] = &[
    "meta-title",
    "meta-artist",
    "cdda-track",
    "bluray-menu",
    "no-bluray-menu",
    "screen-fps",
    "screen-display-id",
    "dshow-vdev",
    "dshow-size",
    "dshow-fps",
    "v4l2-width",
    "v4l2-height",
    "v4l2-fps",
];

/// A disc or capture device to open.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Device {
    mrl: String,
    options: Vec<String>,
}

impl Device {
    /// The device at `mrl`, which must use one of the device schemes; `None` otherwise.
    /// Spaces in it are percent-encoded, as in any URL.
    pub fn new(mrl: &str) -> Option<Self> {
        kind_of(mrl)?;
        Some(Self { mrl: mrl.replace(' ', "%20"), options: Vec::new() })
    }

    /// Adds the libVLC option `option` (`:name=value`) to open it with.
    #[must_use]
    pub fn with_option(mut self, option: impl Into<String>) -> Self {
        self.options.push(option.into());
        self
    }

    /// Names it `title` in the title bar and lists, through libVLC's `meta-title` option.
    #[must_use]
    pub fn titled(self, title: &str) -> Self {
        self.with_option(format!("{TITLE_OPTION}{title}"))
    }

    /// libVLC's MRL for it.
    pub fn mrl(&self) -> &str {
        &self.mrl
    }

    /// The options it opens with, each `:name=value`.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// What kind of device it is.
    pub fn kind(&self) -> DeviceKind {
        kind_of(&self.mrl).expect("a device is only made from a device MRL")
    }

    /// Whether it is a disc rather than a capture device.
    pub fn is_disc(&self) -> bool {
        !matches!(self.kind(), DeviceKind::Camera | DeviceKind::Screen)
    }

    /// Its name: the title it was given, or what kind of device it is.
    pub fn display_name(&self) -> String {
        let titled = self.options.iter().find_map(|option| option.strip_prefix(TITLE_OPTION));
        titled.map_or_else(|| kind_name(self.kind()).to_owned(), str::to_owned)
    }

    /// The stored form; see the module comment.
    pub fn to_stored(&self) -> String {
        let mut text = self.mrl.clone();
        for option in &self.options {
            text.push(' ');
            text.push_str(&escape(option));
        }
        text
    }

    /// The inverse of [`Self::to_stored`]; `None` unless `text` starts with a device MRL.
    /// Options VLC doesn't write itself are dropped.
    pub fn from_stored(text: &str) -> Option<Self> {
        let mut parts = text.split(' ');
        let mut device = Self::new(parts.next()?)?;
        let options = parts.filter(|part| !part.is_empty()).map(unescape);
        device.options = options.filter(|option| is_stored_option(option)).collect();
        Some(device)
    }
}

/// The kind of device `mrl` opens, if it is a device MRL.
pub fn kind_of(mrl: &str) -> Option<DeviceKind> {
    let (scheme, _) = mrl.split_once("://")?;
    SCHEMES.iter().find(|(name, _)| scheme.eq_ignore_ascii_case(name)).map(|&(_, kind)| kind)
}

/// What a device of `kind` is called when it has no title, in the interface language.
pub fn kind_name(kind: DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Dvd => "DVD",
        DeviceKind::BluRay => "Blu-ray",
        DeviceKind::AudioCd => tr!("Audio CD"),
        DeviceKind::Vcd => tr!("Video CD"),
        DeviceKind::Camera => tr!("Camera"),
        DeviceKind::Screen => tr!("Screen"),
    }
}

/// `option` with `%` and spaces percent-encoded.
fn escape(option: &str) -> String {
    option.replace('%', "%25").replace(' ', "%20")
}

/// Whether `option` (`:name` or `:name=value`) is one VLC opens devices with.
fn is_stored_option(option: &str) -> bool {
    let Some(option) = option.strip_prefix(':') else { return false };
    let name = option.split_once('=').map_or(option, |(name, _)| name);
    STORED_OPTIONS.contains(&name)
}

/// The inverse of [`escape`].
fn unescape(text: &str) -> String {
    text.replace("%20", " ").replace("%25", "%")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_device_schemes_make_devices() {
        assert!(Device::new("https://example.com/a.mp4").is_none());
        assert!(Device::new("/dev/rdisk4").is_none());
        assert_eq!(Device::new("DVD:///dev/rdisk4").map(|d| d.kind()), Some(DeviceKind::Dvd));
        assert_eq!(kind_of("screen://"), Some(DeviceKind::Screen));
        assert_eq!(kind_of("cdda:///dev/sr0"), Some(DeviceKind::AudioCd));
    }

    #[test]
    fn stored_devices_round_trip_with_spaces_and_percents() {
        let device = Device::new("bluray:///Volumes/My Film")
            .expect("a device MRL")
            .with_option(":bluray-menu")
            .titled("100% Film: Part 2");
        assert_eq!(device.mrl(), "bluray:///Volumes/My%20Film");
        let stored = device.to_stored();
        assert!(!stored.contains("Film: Part"), "{stored}");
        assert_eq!(Device::from_stored(&stored), Some(device));
    }

    #[test]
    fn stored_text_carries_only_vlcs_own_options() {
        let stored = "dvd:///dev/sr0 :sout=#file{dst=/tmp/x} :meta-title=Film :lua-intf=x :no-x";
        let device = Device::from_stored(stored).expect("a device MRL");
        assert_eq!(device.options(), [":meta-title=Film"]);
    }

    #[test]
    fn names_come_from_the_title_or_the_kind() {
        let track = Device::new("cdda:///dev/rdisk4").expect("a device MRL");
        assert_eq!(track.display_name(), "Audio CD");
        assert!(track.is_disc());
        let titled = track.with_option(":cdda-track=3").titled("Blue in Green");
        assert_eq!(titled.display_name(), "Blue in Green");
        assert!(!Device::new("avcapture://0x1420000").expect("a device MRL").is_disc());
    }
}
