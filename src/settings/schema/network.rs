//! Network settings (Settings → Network, S5.10.1): how much libVLC buffers per kind of source,
//! the HTTP proxy, and how TLS connections are checked.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// Network preferences (S5.10.1). The proxy password isn't here: it lives in the OS keychain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSettings {
    /// "Buffering": a preset, or Custom for the per-source times below.
    #[serde(deserialize_with = "lenient")]
    pub buffering: Buffering,
    /// The per-source times Custom uses.
    #[serde(deserialize_with = "lenient")]
    pub custom_caching: Caching,
    /// "HTTP proxy" as typed; empty uses the system's proxy. Read it through [`proxy_url`].
    #[serde(deserialize_with = "lenient")]
    pub http_proxy: String,
    /// "Verify certificates against the system trust store": off, libVLC's questions about a
    /// certificate it can't verify are answered "accept".
    #[serde(deserialize_with = "lenient")]
    pub verify_certificates: bool,
    /// "Cipher preference".
    #[serde(deserialize_with = "lenient")]
    pub ciphers: CipherPreference,
}

impl Default for NetworkSettings {
    fn default() -> Self {
        Self {
            buffering: Buffering::default(),
            custom_caching: Buffering::Normal.caching(),
            http_proxy: String::new(),
            verify_certificates: true,
            ciphers: CipherPreference::default(),
        }
    }
}

impl NetworkSettings {
    /// The buffering times in use: the preset's, or the custom ones.
    pub fn caching(&self) -> Caching {
        match self.buffering {
            Buffering::Custom => self.custom_caching,
            preset => preset.caching(),
        }
    }
}

/// How long libVLC buffers each kind of source before playing, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Caching {
    /// Files on this machine (`file-caching`).
    #[serde(deserialize_with = "lenient")]
    pub local_files: u32,
    /// DVDs, Blu-rays and audio CDs (`disc-caching`).
    #[serde(deserialize_with = "lenient")]
    pub discs: u32,
    /// Network streams (`network-caching`).
    #[serde(deserialize_with = "lenient")]
    pub network: u32,
    /// Capture devices (`live-caching`).
    #[serde(deserialize_with = "lenient")]
    pub live_capture: u32,
}

impl Default for Caching {
    fn default() -> Self {
        Buffering::Normal.caching()
    }
}

/// "Buffering": how much is buffered, from the lowest latency to the smoothest playback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Buffering {
    /// Starts fastest and keeps live streams closest to live, but stutters on a slow network.
    Lowest,
    /// Less than usual.
    Low,
    /// libVLC's own defaults.
    #[default]
    Normal,
    /// More than usual, for a slow or unsteady network.
    High,
    /// Much more, for a very unsteady network.
    Highest,
    /// The times in [`NetworkSettings::custom_caching`].
    Custom,
}

impl Buffering {
    /// The preset's times; Custom's are the Normal ones it starts from.
    pub const fn caching(self) -> Caching {
        let (local_files, discs, network, live_capture) = match self {
            Self::Lowest => (100, 100, 150, 100),
            Self::Low => (300, 200, 500, 200),
            Self::Normal | Self::Custom => (1000, 300, 1000, 300),
            Self::High => (2000, 1000, 3000, 1000),
            Self::Highest => (5000, 3000, 10_000, 3000),
        };
        Caching { local_files, discs, network, live_capture }
    }
}

/// "Cipher preference": which TLS ciphers are offered (GnuTLS's priority strings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CipherPreference {
    /// The fastest ciphers first.
    Performance,
    /// GnuTLS's usual list.
    #[default]
    Balanced,
    /// Only ciphers of 128 bits or more.
    Secure128,
    /// Only ciphers of 256 bits.
    Secure256,
}

/// "HTTP proxy" as a URL, `http://` added when it has no scheme; `None` when it isn't an http
/// address with a host (or is empty).
pub fn proxy_url(text: &str) -> Option<url::Url> {
    let text = text.trim();
    let with_scheme = if text.contains("://") { text.to_owned() } else { format!("http://{text}") };
    url::Url::parse(&with_scheme)
        .ok()
        .filter(|url| url.scheme() == "http" && url.host_str().is_some_and(|host| !host.is_empty()))
}
