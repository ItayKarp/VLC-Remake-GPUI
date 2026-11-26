//! Reading what libVLC counted about the open media (S8.13.2), on the worker thread.
//!
//! libVLC keeps the counts on the media's input item, so they start again from zero whenever the
//! media is reopened (a switch to software decoding, new filters, a stream started).

use std::ffi::c_int;

use super::api::MediaStats;
use super::decoding::Decoding;
use super::worker::Worker;
use crate::engine::backend::BackendEvent;
use crate::engine::statistics::{Kbps, Statistics, VideoDecoding};

/// Turns libVLC's bytes per microsecond into kilobits per second, as VLC's own statistics do.
const KBPS_PER_BYTE_PER_US: f32 = 8000.0;

impl Worker {
    /// Reports the open media's counts; nothing without media, or when libVLC has none yet.
    pub(super) fn read_statistics(&self) {
        if self.media.is_null() {
            return;
        }
        let mut stats = MediaStats::default();
        // SAFETY: the media is live and `stats` is a `libvlc_media_stats_t` to fill.
        if !unsafe { (self.vlc.api.libvlc_media_get_stats)(self.media, &raw mut stats) } {
            return;
        }
        let decoding = match self.decoding {
            Decoding::Hardware => VideoDecoding::Hardware,
            Decoding::Software => VideoDecoding::Software,
        };
        self.shared.send(BackendEvent::Statistics(statistics(&stats, decoding)));
    }
}

/// The counts in `stats`, for video decoded as `decoding` says.
fn statistics(stats: &MediaStats, decoding: VideoDecoding) -> Statistics {
    Statistics {
        input_rate: kbps(stats.input_bitrate),
        stream_rate: kbps(stats.demux_bitrate),
        decoded_pictures: count(stats.decoded_video),
        shown_pictures: count(stats.displayed_pictures),
        dropped_pictures: count(stats.lost_pictures),
        dropped_audio: count(stats.lost_abuffers),
        decoding,
    }
}

fn kbps(bytes_per_us: f32) -> Kbps {
    Kbps(if bytes_per_us.is_finite() {
        (bytes_per_us * KBPS_PER_BYTE_PER_US).max(0.0)
    } else {
        0.0
    })
}

/// A count libVLC keeps as a C `int`; negative only if it wrapped, which reads as zero.
fn count(value: c_int) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_become_kilobits_and_counts_never_go_negative() {
        let stats = MediaStats {
            input_bitrate: 0.75,
            demux_bitrate: f32::NAN,
            decoded_video: 120,
            displayed_pictures: 118,
            lost_pictures: -1,
            ..MediaStats::default()
        };
        let read = statistics(&stats, VideoDecoding::Software);
        assert_eq!(read.input_rate, Kbps(6000.0));
        assert_eq!(read.stream_rate, Kbps(0.0));
        assert_eq!((read.decoded_pictures, read.shown_pictures), (120, 118));
        assert_eq!(read.dropped_pictures, 0);
        assert_eq!(read.decoding, VideoDecoding::Software);
    }
}
