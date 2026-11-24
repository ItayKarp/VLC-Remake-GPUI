//! What the statistics overlay says (S8.13.2), kept apart from drawing so it can be tested.

use crate::engine::media_info::VideoSize;
use crate::engine::statistics::{Kbps, Statistics, VideoDecoding};
use crate::engine::tracks::Track;

/// Shown for a value not known yet.
pub const UNKNOWN: &str = "—";
/// Kilobits in a megabit.
const KBPS_PER_MBPS: f32 = 1000.0;

/// What the overlay is made from: the player's view of the media playing.
pub struct Sources<'a> {
    /// The video track playing, if any.
    pub video: Option<&'a Track>,
    /// The audio track playing, if any.
    pub audio: Option<&'a Track>,
    /// The size the video shows at.
    pub size: Option<VideoSize>,
    /// How full the input cache is, 0 to 100.
    pub buffer: f32,
    /// What the backend counted; `None` until first read.
    pub counts: Option<&'a Statistics>,
}

/// The overlay's rows, label and value, in order.
pub fn rows(sources: &Sources) -> Vec<(&'static str, String)> {
    let counts = sources.counts;
    vec![
        ("Video", codec(sources.video)),
        ("Audio", audio(sources.audio)),
        ("Resolution", sources.size.map_or_else(unknown, resolution)),
        ("Bitrate", counts.map_or_else(unknown, bitrate)),
        ("Dropped frames", counts.map_or_else(unknown, dropped)),
        ("Buffer", format!("{:.0}%", sources.buffer.clamp(0.0, 100.0))),
        ("Hardware decoding", decoding(sources.video, counts)),
    ]
}

fn unknown() -> String {
    UNKNOWN.into()
}

/// A track's codec, "None" without one.
fn codec(track: Option<&Track>) -> String {
    match track {
        None => "None".into(),
        Some(track) => track.codec.clone().unwrap_or_else(unknown),
    }
}

/// The audio's codec and channels: "AAC · 2 ch".
fn audio(track: Option<&Track>) -> String {
    let codec = codec(track);
    match track.and_then(|track| track.channels) {
        Some(channels) => format!("{codec} · {channels} ch"),
        None => codec,
    }
}

fn resolution(size: VideoSize) -> String {
    format!("{} × {}", size.width, size.height)
}

/// The media's own rate where libVLC knows it, else what arrives: "5.3 Mb/s", "128 kb/s".
fn bitrate(counts: &Statistics) -> String {
    let Kbps(rate) =
        if counts.stream_rate.0 > 0.0 { counts.stream_rate } else { counts.input_rate };
    if rate >= KBPS_PER_MBPS {
        format!("{:.1} Mb/s", rate / KBPS_PER_MBPS)
    } else {
        format!("{rate:.0} kb/s")
    }
}

/// Pictures dropped of those decoded: "3 of 1204".
fn dropped(counts: &Statistics) -> String {
    format!("{} of {}", counts.dropped_pictures, counts.decoded_pictures)
}

/// "On" or "Off" for media with video; "None" without.
fn decoding(video: Option<&Track>, counts: Option<&Statistics>) -> String {
    if video.is_none() {
        return "None".into();
    }
    match counts.map(|counts| counts.decoding) {
        Some(VideoDecoding::Hardware) => "On".into(),
        Some(VideoDecoding::Software) => "Off".into(),
        None => unknown(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tracks::{TrackId, TrackKind};

    fn track(kind: TrackKind, codec: &str, channels: Option<u32>) -> Track {
        Track { codec: Some(codec.into()), channels, ..Track::new(TrackId(1), kind) }
    }

    fn value<'a>(rows: &'a [(&str, String)], label: &str) -> &'a str {
        let row = rows.iter().find(|(shown, _)| *shown == label);
        row.map(|(_, value)| value.as_str()).expect("a row")
    }

    #[test]
    fn a_video_names_its_codecs_size_and_counts() {
        let video = track(TrackKind::Video, "H264", None);
        let audio = track(TrackKind::Audio, "AAC", Some(2));
        let counts = Statistics {
            stream_rate: Kbps(5270.0),
            decoded_pictures: 1204,
            dropped_pictures: 3,
            ..Statistics::default()
        };
        let rows = rows(&Sources {
            video: Some(&video),
            audio: Some(&audio),
            size: Some(VideoSize { width: 1920, height: 1080 }),
            buffer: 100.0,
            counts: Some(&counts),
        });
        assert_eq!(value(&rows, "Video"), "H264");
        assert_eq!(value(&rows, "Audio"), "AAC · 2 ch");
        assert_eq!(value(&rows, "Resolution"), "1920 × 1080");
        assert_eq!(value(&rows, "Bitrate"), "5.3 Mb/s");
        assert_eq!(value(&rows, "Dropped frames"), "3 of 1204");
        assert_eq!(value(&rows, "Buffer"), "100%");
        assert_eq!(value(&rows, "Hardware decoding"), "On");
    }

    #[test]
    fn a_live_stream_before_its_first_count_shows_what_it_knows() {
        let audio = track(TrackKind::Audio, "MP3", None);
        let rows = rows(&Sources {
            video: None,
            audio: Some(&audio),
            size: None,
            buffer: 42.4,
            counts: None,
        });
        assert_eq!(value(&rows, "Video"), "None");
        assert_eq!(value(&rows, "Audio"), "MP3");
        assert_eq!(value(&rows, "Bitrate"), UNKNOWN);
        assert_eq!(value(&rows, "Buffer"), "42%");
        assert_eq!(value(&rows, "Hardware decoding"), "None");
    }

    #[test]
    fn a_stream_without_its_own_rate_shows_what_arrives() {
        let counts = Statistics { input_rate: Kbps(128.4), ..Statistics::default() };
        assert_eq!(bitrate(&counts), "128 kb/s");
    }
}
