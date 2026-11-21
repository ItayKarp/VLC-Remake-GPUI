//! Releasing a backend without letting it block quitting (S1.2.6). A hung network stream can
//! make libVLC's stop wait forever, so the release runs on its own thread and is abandoned after
//! [`SHUTDOWN_TIMEOUT`].

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use super::backend::Backend;

/// How long quitting waits for a backend to release.
pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

/// How a backend shutdown went.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shutdown {
    /// The backend stopped and released everything in time.
    Clean,
    /// The backend didn't finish in time; its thread was left behind for the OS to reap.
    Abandoned,
}

/// Shuts `backend` down, waiting at most `timeout` for it.
pub fn shut_down(backend: Box<dyn Backend>, timeout: Duration) -> Shutdown {
    let (done, finished) = mpsc::channel();
    let spawned = thread::Builder::new().name("engine-shutdown".into()).spawn(move || {
        backend.shut_down();
        let _ = done.send(());
    });
    if let Err(error) = spawned {
        tracing::error!("engine: couldn't start the shutdown thread: {error}");
        return Shutdown::Abandoned;
    }
    if finished.recv_timeout(timeout).is_ok() {
        return Shutdown::Clean;
    }
    tracing::warn!("engine: the backend didn't shut down within {timeout:?}");
    Shutdown::Abandoned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::backend;
    use crate::engine::mock::{MockBackend, MockMedia};

    #[test]
    fn a_prompt_backend_shuts_down_cleanly() {
        let (events, _receiver) = backend::channel();
        let (mock, _handle) = MockBackend::new(events, MockMedia::default());
        assert_eq!(shut_down(Box::new(mock), SHUTDOWN_TIMEOUT), Shutdown::Clean);
    }

    #[test]
    fn a_hung_backend_is_abandoned_after_the_timeout() {
        let (events, _receiver) = backend::channel();
        let (mock, _handle) = MockBackend::new(events, MockMedia::default());
        let mock = mock.with_shutdown_delay(Duration::from_secs(5));
        let shutdown = shut_down(Box::new(mock), Duration::from_millis(50));
        assert_eq!(shutdown, Shutdown::Abandoned);
    }
}
