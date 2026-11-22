//! How the player's libVLC instance connects and decodes, beyond its audio and subtitle style:
//! buffering, the HTTP proxy and TLS (Settings → Network, S5.10.1), and decoding, priority and
//! the clock (Settings → Performance, S5.10.2). Set once at startup, so changes apply at the
//! next launch.

/// libVLC's own defaults for what isn't covered by a preset elsewhere.
const DEFAULT_CLOCK_JITTER_MS: u32 = 5000;
const DEFAULT_CLOCK_AVERAGE: u32 = 40;

/// The player instance's network and performance options.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent switches, each its own row in Settings → Network or Performance"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemOptions {
    /// How long each kind of source buffers.
    pub caching: Caching,
    /// The HTTP proxy; `None` uses the system's.
    pub proxy: Option<Proxy>,
    /// Whether certificates libVLC can't verify are refused (on) or accepted.
    pub verify_certificates: bool,
    /// Which TLS ciphers are offered.
    pub ciphers: Ciphers,
    /// Whether the GPU may decode video.
    pub hardware_decoding: bool,
    /// The decoder's post-processing quality, 0 (none) to 6.
    pub post_processing: u8,
    /// Which frames skip H.264's loop filter, 0 (none) to 4 (all).
    pub skip_loop_filter: u8,
    /// How many threads decode video; 0 lets the decoder choose.
    pub decoding_threads: u32,
    /// Whether late pictures are dropped to keep sound and picture together.
    pub drop_late_frames: bool,
    /// Whether libVLC's threads run at real-time priority.
    pub real_time_priority: bool,
    /// Added to the real-time threads' priority.
    pub priority_offset: i32,
    /// The encoder tried first; `None` lets libVLC pick.
    pub encoder: Option<&'static str>,
    /// The decoder tried first; `None` lets libVLC pick.
    pub decoder: Option<&'static str>,
    /// Whether the clock follows a live source; `None` lets libVLC decide per source.
    pub clock_synchronisation: Option<bool>,
    /// How far a source's clock may wander, in milliseconds.
    pub clock_jitter_ms: u32,
    /// How many clock references are averaged.
    pub clock_average: u32,
    /// Whether libVLC looks for its plugins again instead of trusting its cache.
    pub rescan_plugins: bool,
}

impl Default for SystemOptions {
    fn default() -> Self {
        Self {
            caching: Caching::default(),
            proxy: None,
            verify_certificates: true,
            ciphers: Ciphers::default(),
            hardware_decoding: true,
            post_processing: 0,
            skip_loop_filter: 0,
            decoding_threads: 0,
            drop_late_frames: true,
            real_time_priority: false,
            priority_offset: 0,
            encoder: None,
            decoder: None,
            clock_synchronisation: None,
            clock_jitter_ms: DEFAULT_CLOCK_JITTER_MS,
            clock_average: DEFAULT_CLOCK_AVERAGE,
            rescan_plugins: false,
        }
    }
}

/// How long each kind of source buffers before playing, in milliseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caching {
    /// Files on this machine.
    pub local_files: u32,
    /// DVDs, Blu-rays and audio CDs.
    pub discs: u32,
    /// Network streams.
    pub network: u32,
    /// Capture devices.
    pub live_capture: u32,
}

impl Default for Caching {
    /// libVLC's own defaults.
    fn default() -> Self {
        Self { local_files: 1000, discs: 300, network: 1000, live_capture: 300 }
    }
}

/// An HTTP proxy.
#[derive(Clone, PartialEq, Eq)]
pub struct Proxy {
    /// Its address, `http://[user@]host[:port]/`.
    pub url: String,
    /// Its password, from the keychain.
    pub password: Option<String>,
}

impl std::fmt::Debug for Proxy {
    /// Never prints the password, so it can't reach a log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proxy")
            .field("url", &self.url)
            .field("password", &self.password.as_ref().map(|_| "…"))
            .finish()
    }
}

/// Which TLS ciphers are offered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ciphers {
    /// The fastest first.
    Performance,
    /// The TLS library's usual list.
    #[default]
    Normal,
    /// 128 bits or more.
    Secure128,
    /// 256 bits.
    Secure256,
}
