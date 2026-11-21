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

use backend::{Backend, EventSender};

pub use libvlc::runtime::prepare_environment;

/// The backend to play with: libVLC, or, when it can't be loaded, one that fails every open
/// with a clear error. Pass it to [`Player::new`](player::Player::new).
pub fn default_backend(events: EventSender) -> Box<dyn Backend> {
    match libvlc::LibVlcBackend::new(events.clone()) {
        Ok(backend) => Box::new(backend),
        Err(error) => {
            tracing::error!("engine: libVLC is unavailable: {error:#}");
            Box::new(unavailable::UnavailableBackend::new(events))
        }
    }
}
