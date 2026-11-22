//! Whether VLC may run real-time threads here ("Give VLC real-time priority", S5.10.2).
//!
//! On Linux libVLC's `--rt-priority` starts every thread round-robin, which the kernel refuses
//! without `CAP_SYS_NICE` or an `rtprio` limit, and then libVLC can't start its threads at all.
//! So the app asks first, by trying it on a thread of its own: refused, it leaves the option off
//! and the row says why. Windows' high priority class needs no rights, and macOS's libVLC has no
//! real-time option, so the row isn't shown there.

/// Why real-time priority can't be had here, or `None` where it can (or doesn't apply).
pub fn refusal() -> Option<&'static str> {
    #[cfg(target_os = "linux")]
    {
        static REFUSAL: std::sync::OnceLock<Option<&'static str>> = std::sync::OnceLock::new();
        *REFUSAL.get_or_init(linux::probe)
    }
    #[cfg(not(target_os = "linux"))]
    None
}

#[cfg(target_os = "linux")]
mod linux {
    /// What the row says when the kernel refuses.
    const NOT_PERMITTED: &str = "This system doesn't let VLC use real-time priority, so it runs \
                                 as usual. Give your user an rtprio limit to allow it.";
    /// What the row says when the probe itself fails.
    const UNKNOWN: &str = "VLC couldn't check whether real-time priority is allowed, so it runs \
                           as usual.";

    /// Tries real-time scheduling on a short-lived thread, which is dropped right after.
    pub(super) fn probe() -> Option<&'static str> {
        let result = std::thread::Builder::new()
            .name("rt-probe".into())
            .spawn(try_round_robin)
            .ok()
            .and_then(|thread| thread.join().ok());
        match result {
            Some(0) => None,
            Some(libc::EPERM) => {
                tracing::warn!("real-time priority isn't permitted for this user");
                Some(NOT_PERMITTED)
            }
            other => {
                tracing::warn!("real-time priority probe failed: {other:?}");
                Some(UNKNOWN)
            }
        }
    }

    /// Switches the calling thread to round-robin at the lowest real-time priority; returns
    /// `pthread_setschedparam`'s error code.
    fn try_round_robin() -> i32 {
        // SAFETY: only queries the scheduler's range.
        let lowest = unsafe { libc::sched_get_priority_min(libc::SCHED_RR) };
        // SAFETY: `sched_param` is plain integers, so all zeros is valid; some libcs add fields.
        let mut param: libc::sched_param = unsafe { std::mem::zeroed() };
        param.sched_priority = lowest;
        // SAFETY: changes the calling thread only, which ends right after.
        unsafe {
            libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_RR, &raw const param)
        }
    }
}
