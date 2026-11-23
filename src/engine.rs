//! The playback engine (S1.2): a [`Player`](player::Player) GPUI entity that owns a playback
//! [`Backend`](backend::Backend) and runs the playback state machine.
//!
//! - [`state`]: the states and the transitions allowed between them.
//! - [`backend`]: the interface a backend implements, and the channel its events reach the
//!   foreground thread through.
//! - [`player`]: commands in, [`PlayerEvent`](player::PlayerEvent)s out; stale events from a
//!   cancelled open are dropped, seeks and frame steps are coalesced, and positions are
//!   throttled for the UI.
//! - [`libvlc`]: the real backend; [`default_backend`] falls back to one that reports
//!   [`ErrorKind::BackendUnavailable`](state::ErrorKind::BackendUnavailable) when libVLC is
//!   missing.
//! - [`mock`]: a backend without media, for tests and for building UI without libVLC.
//! - [`tracks`]: the audio, video and subtitle tracks, their names, cycling them and picking
//!   them by the preferred [`language`]s (S1.6).
//! - [`audio`]: output devices, speaker setup and hot-plug routing (S1.5).
//! - [`video`]: the sink interface decoded pictures are delivered through (S1.4).
//! - [`preview`]: seek-bar preview pictures from a second, muted decoder (S2.3.3).
//! - [`lengths`]: how long queued media is, read without playing it (S3.2.2).
//! - [`media_options`]: how new media opens: stream quality, disc menu language and seek
//!   precision (S5.7).
//! - [`tags`]: a file's tags, tracks and cover, read without playing it (S4.3.1).
//! - [`subtitle_sync`]: a subtitle track's delay, speed and duration factor (S6.3).
//! - [`transform`]: flipping and rotating the picture (S7.2).
//! - [`colors`]: the color filters (S7.4).
//! - [`thumbnail`]: one picture of a video at a chosen time, for library thumbnails (S4.4.1).
//! - [`waveform`]: a file's loudness along its length, for the waveform seek bar (S7.8.2).
//! - [`recording`]: what becomes of recording the media playing (S8.6).
//! - [`navigation`]: a stream's programs, a disc's titles and its menus (S8.9).
//! - [`discs`]: an audio CD's tracks, read without playing it (S8.9.1).

pub mod audio;
pub mod backend;
mod coalesce;
pub mod colors;
pub mod cues;
pub mod discs;
pub mod hdr;
pub mod language;
pub mod lengths;
pub mod libvlc;
pub mod media_info;
pub mod media_options;
pub mod mock;
pub mod navigation;
pub mod player;
mod position;
mod preflight;
pub mod preview;
pub mod recording;
pub mod shutdown;
pub mod sidecar;
pub mod state;
pub mod subtitle_style;
pub mod subtitle_sync;
pub mod subtitle_text;
pub mod system_options;
pub mod tags;
#[cfg(test)]
mod tests;
pub mod thumbnail;
pub mod tracks;
pub mod transform;
mod unavailable;
pub mod values;
pub mod video;
pub mod waveform;

use std::sync::Arc;

use audio::AudioConfig;
use backend::{Backend, EventSender};
use subtitle_style::SubtitleStyle;
use video::VideoSink;

pub use libvlc::runtime::prepare_environment;
pub use libvlc::{loaded_version as loaded_libvlc_version, version as libvlc_version};

/// The backend to play with, sending pictures to `video` and set up with `audio` and the
/// `subtitles` style: libVLC, or, when it can't be loaded, one that fails every open with a clear
/// error. Pass it to [`Player::new`](player::Player::new).
pub fn default_backend(
    video: Arc<dyn VideoSink>,
    audio: AudioConfig,
    subtitles: SubtitleStyle,
) -> impl FnOnce(EventSender) -> Box<dyn Backend> {
    move |events| new_backend(events, video, audio, &subtitles)
}

fn new_backend(
    events: EventSender,
    video: Arc<dyn VideoSink>,
    audio: AudioConfig,
    subtitles: &SubtitleStyle,
) -> Box<dyn Backend> {
    match libvlc::LibVlcBackend::new(events.clone(), Some(video), audio, subtitles) {
        Ok(backend) => Box::new(backend),
        Err(error) => {
            tracing::error!("engine: libVLC is unavailable: {error:#}");
            Box::new(unavailable::UnavailableBackend::new(events))
        }
    }
}
