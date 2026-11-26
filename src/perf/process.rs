//! What the OS says this process uses: CPU time and resident memory.

use std::time::Duration;

/// The CPU time every thread of this process has used, user and system; `None` where unknown.
#[cfg(unix)]
pub fn cpu_time() -> Option<Duration> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: `getrusage` fills the struct it is given and reports failure by its result.
    let usage = unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return None;
        }
        usage.assume_init()
    };
    Some(timeval(usage.ru_utime) + timeval(usage.ru_stime))
}

/// The CPU time this process has used; not measured on this OS.
#[cfg(not(unix))]
pub fn cpu_time() -> Option<Duration> {
    None
}

#[cfg(unix)]
fn timeval(time: libc::timeval) -> Duration {
    let secs = u64::try_from(time.tv_sec).unwrap_or(0);
    let micros = u64::try_from(time.tv_usec).unwrap_or(0);
    Duration::from_secs(secs) + Duration::from_micros(micros)
}

/// The memory this process holds in RAM now, in bytes; `None` where unknown.
#[cfg(target_os = "macos")]
pub fn resident_bytes() -> Option<u64> {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::uninit();
    let size = i32::try_from(size_of::<libc::proc_taskinfo>()).ok()?;
    // SAFETY: `proc_pidinfo` writes at most `size` bytes into `info` and returns how many it
    // wrote; the struct is only read when it wrote all of it.
    let info = unsafe {
        let written = libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        );
        if written != size {
            return None;
        }
        info.assume_init()
    };
    Some(info.pti_resident_size)
}

/// The memory this process holds in RAM now, in bytes, from `/proc/self/statm`.
#[cfg(target_os = "linux")]
pub fn resident_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    // SAFETY: `sysconf` only reads a system constant.
    let page_size = u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok()?;
    Some(pages * page_size)
}

/// The memory this process holds in RAM; not measured on this OS.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn resident_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[test]
    fn this_process_uses_some_memory_and_cpu() {
        assert!(resident_bytes().is_some_and(|bytes| bytes > 1_000_000));
        let spin = std::time::Instant::now();
        while spin.elapsed() < Duration::from_millis(20) {
            std::hint::black_box(0);
        }
        assert!(cpu_time().is_some_and(|time| time > Duration::ZERO));
    }
}
