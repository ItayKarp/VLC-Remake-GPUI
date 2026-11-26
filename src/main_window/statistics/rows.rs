//! What the statistics overlay says (S8.13.2), kept apart from drawing so it can be tested.

use crate::engine::media_info::VideoSize;
use crate::engine::statistics::{Kbps, Statistics, VideoDecoding};
use crate::engine::tracks::Track;
use crate::i18n;

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
    /// Whether the backend counts at all ("Collect playback statistics", S9.6.1).
    pub collecting: bool,
}

/// The overlay's rows, label and value, in order.
pub fn rows(sources: &Sources) -> Vec<(&'static str, String)> {
    let counts = sources.counts;
    let counted = |show: fn(&Statistics) -> String| match counts {
        _ if !sources.collecting => tr!("Not collected").to_owned(),
        Some(counts) => show(counts),
        None => unknown(),
    };
    vec![
        (tr!("Video"), codec(sources.video)),
        (tr!("Audio"), audio(sources.audio)),
        (tr!("Resolution"), sources.size.map_or_else(unknown, resolution)),
        (tr!("Bitrate"), counted(bitrate)),
        (tr!("Dropped frames"), counted(dropped)),
        (tr!("Buffer"), buffer(sources.buffer)),
        (tr!("Hardware decoding"), decoding(sources.video, counts)),
    ]
}

/// How full the cache is: "42%".
fn buffer(percent: f32) -> String {
    format!("{}%", i18n::decimal(f64::from(percent.clamp(0.0, 100.0)), 0))
}

fn unknown() -> String {
    UNKNOWN.into()
}

/// A track's codec, "None" without one.
fn codec(track: Option<&Track>) -> String {
    match track {
        None => tr!("None").into(),
        Some(track) => track.codec.clone().unwrap_or_else(unknown),
    }
}

/// The audio's codec and channels: "AAC · 2 ch".
fn audio(track: Option<&Track>) -> String {
    let codec = codec(track);
    match track.and_then(|track| track.channels) {
        Some(channels) => tr!("{codec} · {channels} ch", codec, channels),
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
        tr!("{rate} Mb/s", rate = i18n::decimal(f64::from(rate / KBPS_PER_MBPS), 1))
    } else {
        tr!("{rate} kb/s", rate = i18n::decimal(f64::from(rate), 0))
    }
}

/// Pictures dropped of those decoded: "3 of 1204".
fn dropped(counts: &Statistics) -> String {
    let (dropped, decoded) = (counts.dropped_pictures, counts.decoded_pictures);
    tr!("{dropped} of {decoded}", dropped, decoded)
}

/// "On" or "Off" for media with video; "None" without.
fn decoding(video: Option<&Track>, counts: Option<&Statistics>) -> String {
    if video.is_none() {
        return tr!("None").into();
    }
    match counts.map(|counts| counts.decoding) {
        Some(VideoDecoding::Hardware) => tr!("On").into(),
        Some(VideoDecoding::Software) => tr!("Off").into(),
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
            collecting: true,
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
            collecting: true,
        });
        assert_eq!(value(&rows, "Video"), "None");
        assert_eq!(value(&rows, "Audio"), "MP3");
        assert_eq!(value(&rows, "Bitrate"), UNKNOWN);
        assert_eq!(value(&rows, "Buffer"), "42%");
        assert_eq!(value(&rows, "Hardware decoding"), "None");
    }

    #[test]
    fn counts_left_uncollected_say_so() {
        let video = track(TrackKind::Video, "H264", None);
        let counts = Statistics::default();
        let rows = rows(&Sources {
            video: Some(&video),
            audio: None,
            size: None,
            buffer: 0.0,
            counts: Some(&counts),
            collecting: false,
        });
        assert_eq!(value(&rows, "Bitrate"), "Not collected");
        assert_eq!(value(&rows, "Dropped frames"), "Not collected");
    }

    #[test]
    fn a_stream_without_its_own_rate_shows_what_arrives() {
        let counts = Statistics { input_rate: Kbps(128.4), ..Statistics::default() };
        assert_eq!(bitrate(&counts), "128 kb/s");
    }
}
