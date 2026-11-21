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
//! - [`video`]: the sink interface decoded pictures are delivered through (S1.4).

pub mod backend;
mod coalesce;
pub mod libvlc;
pub mod media_info;
pub mod mock;
pub mod player;
mod position;
mod preflight;
pub mod shutdown;
pub mod state;
#[cfg(test)]
mod tests;
mod unavailable;
pub mod values;
pub mod video;

use std::sync::Arc;

use backend::{Backend, EventSender};
use video::VideoSink;

pub use libvlc::runtime::prepare_environment;

/// The backend to play with, sending pictures to `video`: libVLC, or, when it can't be loaded,
/// one that fails every open with a clear error. Pass it to [`Player::new`](player::Player::new).
pub fn default_backend(video: Arc<dyn VideoSink>) -> impl FnOnce(EventSender) -> Box<dyn Backend> {
    move |events| new_backend(events, video)
}

fn new_backend(events: EventSender, video: Arc<dyn VideoSink>) -> Box<dyn Backend> {
    match libvlc::LibVlcBackend::new(events.clone(), Some(video)) {
        Ok(backend) => Box::new(backend),
        Err(error) => {
            tracing::error!("engine: libVLC is unavailable: {error:#}");
            Box::new(unavailable::UnavailableBackend::new(events))
        }
    }
}
