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
mod linux;
