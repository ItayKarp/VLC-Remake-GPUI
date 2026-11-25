//! What a measured run reads from the world: the time, the process's CPU and memory, and the
//! frames GPUI drew. [`System`] reads the real ones; tests give a [`super::session::Session`]
//! their own.

use std::time::{Duration, Instant};

use super::frames::Frame;
use super::process;

/// The readings a measured run takes.
pub trait Probe: Send + Sync {
    /// The time now.
    fn now(&self) -> Instant;

    /// The CPU time the process has used; `None` where the OS doesn't say.
    fn cpu_time(&self) -> Option<Duration>;

    /// The memory the process holds in RAM now, in bytes; `None` where the OS doesn't say.
    fn resident_bytes(&self) -> Option<u64>;

    /// The frames the window with id `window` drew since the last call, each with when its draw
    /// started. The first call starts the frame timing and returns nothing.
    fn frames(&self, window: u64) -> Vec<(Frame, Instant)>;
}

/// The real clock, the OS's figures, and GPUI's frame timing when built with the `perf`
/// feature (without it, no frames).
#[derive(Clone, Copy, Debug, Default)]
pub struct System;

impl Probe for System {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn cpu_time(&self) -> Option<Duration> {
        process::cpu_time()
    }

    fn resident_bytes(&self) -> Option<u64> {
        process::resident_bytes()
    }

    #[cfg(feature = "perf")]
    fn frames(&self, window: u64) -> Vec<(Frame, Instant)> {
        super::collector::take(window)
    }

    #[cfg(not(feature = "perf"))]
    fn frames(&self, _window: u64) -> Vec<(Frame, Instant)> {
        Vec::new()
    }
}
