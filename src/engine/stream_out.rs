//! Streaming out (S8.12): the media playing sent to the network over HTTP, RTSP or UDP, and the
//! libVLC options that do it.
//!
//! The stream is libVLC's `sout` on the player's own media, so pausing, seeking and track
//! choices reach viewers too. A `duplicate` sends one copy to the screen and speakers when the
//! media also plays here, and another through `transcode` (H.264 and AAC, as nearly every
//! player takes them) to the network: an MPEG-TS over HTTP or UDP, or RTP set up by RTSP.
//! Subtitles are burned into the streamed picture, since few players read them from a stream.
//!
//! The copy played here has picture and subtitles but no sound: libVLC's `display` output opens
//! an audio output of its own, out of reach of the player's volume, mute and device, so its sound
//! could play however the user set them. Its picture trails the media by half a second, which
//! keeps it from going black while the encoder starts.
//!
//! HTTP and RTSP wait for viewers on a port of this computer; UDP sends to one address. Shared
//! with the network, they ask for an [`AccessCode`] that the stream's link carries (S9.7.4).

mod access;
mod host;

use std::net::{IpAddr, SocketAddr};

use crate::settings::{EncodingPreset, RecordingSettings, Tuning};

pub use access::AccessCode;
pub use host::{is_port_free, network_address};

/// The path HTTP and RTSP viewers ask for.
pub const PATH: &str = "/stream";
/// Pictures are encoded on threads of their own, so encoding holds back the screen's copy less.
const ENCODER_THREADS: u32 = 2;
/// How far the screen's copy trails the media, in ms. While the encoder starts, pictures reach
/// the screen late and in bursts; with libVLC's 100 ms the screen dropped them all and stayed
/// black for seconds.
const SCREEN_DELAY_MS: u32 = 500;
/// The bit rate sound is encoded at, in kb/s.
const AUDIO_KBPS: u32 = 128;
/// Streamed sound is stereo at 48 kHz, which every player takes.
const AUDIO_CHANNELS: u32 = 2;
const AUDIO_RATE: u32 = 48_000;

/// How viewers get the stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Protocol {
    /// An MPEG-TS a viewer fetches from this computer, as `http://…/stream`.
    #[default]
    Http,
    /// RTP a viewer sets up with RTSP, as `rtsp://…/stream`.
    Rtsp,
    /// An MPEG-TS sent to one address, which a player there opens as `udp://@:port`.
    Udp,
}

impl Protocol {
    /// Every protocol, as the dialog lists them.
    pub const ALL: [Self; 3] = [Self::Http, Self::Rtsp, Self::Udp];

    /// Its name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Http => "HTTP",
            Self::Rtsp => "RTSP",
            Self::Udp => "UDP",
        }
    }

    /// The port it uses unless told otherwise.
    pub fn default_port(self) -> u16 {
        match self {
            Self::Http => 8080,
            Self::Rtsp => 8554,
            Self::Udp => 1234,
        }
    }

    /// Whether viewers connect to this computer, which listens on a port, rather than it
    /// sending to them.
    pub fn listens(self) -> bool {
        self != Self::Udp
    }
}

/// How much picture the stream carries, for the network it goes over.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StreamQuality {
    /// Up to 1080 lines at 6 Mb/s.
    High,
    /// Up to 720 lines at 3 Mb/s.
    #[default]
    Medium,
    /// Up to 480 lines at 1 Mb/s, for a slow network.
    Low,
}

impl StreamQuality {
    /// Every quality, best first.
    pub const ALL: [Self; 3] = [Self::High, Self::Medium, Self::Low];

    /// Its name in the dialog.
    pub fn name(self) -> &'static str {
        match self {
            Self::High => tr!("High – up to 1080p, 6 Mb/s"),
            Self::Medium => tr!("Medium – up to 720p, 3 Mb/s"),
            Self::Low => tr!("Low – up to 480p, 1 Mb/s"),
        }
    }

    /// The video bit rate, in kb/s.
    fn video_kbps(self) -> u32 {
        match self {
            Self::High => 6000,
            Self::Medium => 3000,
            Self::Low => 1000,
        }
    }

    /// The most lines the picture keeps; taller pictures are scaled down.
    fn max_height(self) -> u32 {
        match self {
            Self::High => 1080,
            Self::Medium => 720,
            Self::Low => 480,
        }
    }
}

/// What the stream carries: at least sound or picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contents {
    /// Sound and picture, with the subtitle showing burned into the picture or not.
    Everything {
        /// Whether the subtitle showing is burned in.
        subtitles: bool,
    },
    /// The picture only.
    PictureOnly {
        /// Whether the subtitle showing is burned in.
        subtitles: bool,
    },
    /// The sound only.
    SoundOnly,
}

impl Contents {
    /// What Settings → Recording's "Include audio", "Include video" and "Include subtitles"
    /// send; `None` when neither sound nor picture is.
    pub fn from_settings(recording: &RecordingSettings) -> Option<Self> {
        let subtitles = recording.stream_subtitles;
        match (recording.stream_audio, recording.stream_video) {
            (true, true) => Some(Self::Everything { subtitles }),
            (false, true) => Some(Self::PictureOnly { subtitles }),
            (true, false) => Some(Self::SoundOnly),
            (false, false) => None,
        }
    }

    fn has_sound(self) -> bool {
        !matches!(self, Self::PictureOnly { .. })
    }

    fn has_picture(self) -> bool {
        self != Self::SoundOnly
    }

    fn has_subtitles(self) -> bool {
        matches!(self, Self::Everything { subtitles: true } | Self::PictureOnly { subtitles: true })
    }

    /// The `duplicate` selection keeping these streams from the stream's copy; `None` keeps
    /// every one. `duplicate` reads one kind of stream per selection, which covers each case.
    fn selection(self) -> Option<&'static str> {
        match self {
            Self::Everything { subtitles: true } => None,
            Self::Everything { subtitles: false } => Some("nospu"),
            Self::PictureOnly { subtitles: true } => Some("noaudio"),
            Self::PictureOnly { subtitles: false } => Some("video"),
            Self::SoundOnly => Some("audio"),
        }
    }
}

/// How the picture is encoded: Settings → Recording's "Encoding preset" and "Tuning".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Encoding {
    /// x264's speed against size.
    pub preset: EncodingPreset,
    /// What x264 tunes for.
    pub tuning: Tuning,
}

/// Whether the media streaming also plays on this computer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Local {
    /// Its picture shows here too, without sound (see the module comment).
    #[default]
    PlaysHere,
    /// Only viewers get it.
    StreamOnly,
}

/// A stream of the media playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamTarget {
    /// How viewers get it.
    pub protocol: Protocol,
    /// Where HTTP and RTSP listen (`127.0.0.1` for this computer only, `0.0.0.0` for every
    /// network it is on), or where UDP sends to.
    pub address: SocketAddr,
    /// The address viewers reach it at, for its link: the listening address, or this
    /// computer's own on its network when it listens on every one.
    pub viewer_host: IpAddr,
    /// How much picture it carries.
    pub quality: StreamQuality,
    /// What it carries.
    pub contents: Contents,
    /// How the picture is encoded.
    pub encoding: Encoding,
    /// Whether the media plays here too.
    pub local: Local,
    /// The code HTTP and RTSP viewers sign in with; `None` lets anyone who can reach the port
    /// watch, which only a stream for this computer alone should do.
    pub access: Option<AccessCode>,
}

impl StreamTarget {
    /// The link viewers open: `http://…/stream`, `rtsp://…/stream`, or for UDP what a player
    /// at the address opens.
    pub fn url(&self) -> String {
        let port = self.address.port();
        let host = SocketAddr::new(self.viewer_host, port);
        let sign_in = self
            .access
            .map_or_else(String::new, |code| format!("{}:{}@", access::VIEWER, code.as_str()));
        match self.protocol {
            Protocol::Http => format!("http://{sign_in}{host}{PATH}"),
            Protocol::Rtsp => format!("rtsp://{sign_in}{host}{PATH}"),
            Protocol::Udp => format!("udp://@:{port}"),
        }
    }

    /// Whether it listens on every network this computer is on, so anyone there can watch.
    pub fn is_open_to_network(&self) -> bool {
        self.protocol.listens() && !self.address.ip().is_loopback()
    }

    /// The libVLC media options that stream the media this way.
    pub fn media_options(&self) -> Vec<String> {
        let mut options = vec![
            format!(":sout={}", self.chain()),
            // Only the tracks chosen, as playing does; otherwise every audio track would play.
            ":no-sout-all".to_owned(),
            format!(":sout-x264-preset={}", preset_name(self.encoding.preset)),
        ];
        options
            .extend(tune_name(self.encoding.tuning).map(|tune| format!(":sout-x264-tune={tune}")));
        options
    }

    /// The libVLC variables that make HTTP and RTSP viewers sign in with the access code, as
    /// `(name, value)`. libVLC's stream output belongs to the player, not the media, so they are
    /// the player's, not media options; each is empty without a code, so no earlier stream's
    /// code stays.
    pub fn sign_in_vars(&self) -> [(&'static str, &str); 4] {
        let (user, password) =
            self.access.as_ref().map_or(("", ""), |code| (access::VIEWER, code.as_str()));
        let (http, rtsp) = match self.protocol {
            Protocol::Http => ((user, password), ("", "")),
            Protocol::Rtsp => (("", ""), (user, password)),
            Protocol::Udp => (("", ""), ("", "")),
        };
        [
            ("sout-http-user", http.0),
            ("sout-http-pwd", http.1),
            ("sout-rtsp-user", rtsp.0),
            ("sout-rtsp-pwd", rtsp.1),
        ]
    }

    /// The `sout` chain: the screen when it plays here, and the stream's own copy.
    fn chain(&self) -> String {
        let display = match self.local {
            // The screen's copy has no sound: `display` opens an audio output of its own, which
            // the player's volume, mute and device can't reach.
            Local::PlaysHere => format!("dst=display{{delay={SCREEN_DELAY_MS}}},select=noaudio,"),
            Local::StreamOnly => String::new(),
        };
        let selection = self
            .contents
            .selection()
            .map_or_else(String::new, |select| format!(",select={select}"));
        // Quoted: an unquoted value ends at its first closing brace, before `:std{…}`.
        format!("#duplicate{{{display}dst=\"{}\"{selection}}}", self.network_branch())
    }

    /// Encoding, then sending to the network.
    fn network_branch(&self) -> String {
        let destination = match self.protocol {
            Protocol::Http => format!("std{{access=http,mux=ts,dst={}{PATH}}}", self.address),
            Protocol::Rtsp => format!("rtp{{sdp=rtsp://{}{PATH}}}", self.address),
            Protocol::Udp => format!("std{{access=udp,mux=ts,dst={}}}", self.address),
        };
        format!("transcode{{{}}}:{destination}", self.codecs())
    }

    /// The `transcode` options for what the stream carries.
    fn codecs(&self) -> String {
        let mut codecs = Vec::new();
        if self.contents.has_picture() {
            let quality = self.quality;
            codecs.push(format!(
                "vcodec=h264,vb={},maxheight={},threads={ENCODER_THREADS}",
                quality.video_kbps(),
                quality.max_height()
            ));
        }
        if self.contents.has_subtitles() {
            codecs.push("soverlay".to_owned());
        }
        if self.contents.has_sound() {
            codecs.push(format!(
                "acodec=mp4a,ab={AUDIO_KBPS},channels={AUDIO_CHANNELS},samplerate={AUDIO_RATE}"
            ));
        }
        codecs.join(",")
    }
}

/// x264's name for `preset`.
fn preset_name(preset: EncodingPreset) -> &'static str {
    match preset {
        EncodingPreset::Ultrafast => "ultrafast",
        EncodingPreset::Fast => "fast",
        EncodingPreset::Medium => "medium",
        EncodingPreset::Slow => "slow",
        EncodingPreset::Veryslow => "veryslow",
    }
}

/// x264's name for `tuning`; `None` tunes for nothing in particular.
fn tune_name(tuning: Tuning) -> Option<&'static str> {
    match tuning {
        Tuning::None => None,
        Tuning::Film => Some("film"),
        Tuning::Animation => Some("animation"),
        Tuning::Grain => Some("grain"),
        Tuning::StillImage => Some("stillimage"),
        Tuning::ZeroLatency => Some("zerolatency"),
    }
}

#[cfg(test)]
mod tests;
