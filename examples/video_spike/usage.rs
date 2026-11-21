//! The process's own CPU time and peak memory, read from the OS.

use std::time::Duration;

/// User plus system CPU time of the whole process, libVLC's decoder threads included.
#[cfg(unix)]
pub fn process_cpu_time() -> Option<Duration> {
    let usage = resource_usage()?;
    let time = |t: libc::timeval| Duration::new(t.tv_sec as u64, t.tv_usec as u32 * 1000);
    Some(time(usage.ru_utime) + time(usage.ru_stime))
}

/// The most memory the process has held in RAM at once, in MB.
#[cfg(unix)]
pub fn peak_rss_mb() -> Option<f64> {
    // macOS reports bytes, Linux kibibytes.
    let unit = if cfg!(target_os = "macos") { 1.0 } else { 1024.0 };
    Some(resource_usage()?.ru_maxrss as f64 * unit / (1024.0 * 1024.0))
}

#[cfg(unix)]
fn resource_usage() -> Option<libc::rusage> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: `usage` is a writable rusage; getrusage fills it on success.
    let status = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    // SAFETY: getrusage succeeded, so the struct is initialized.
    (status == 0).then(|| unsafe { usage.assume_init() })
}

/// User plus system CPU time of the whole process, libVLC's decoder threads included.
#[cfg(windows)]
pub fn process_cpu_time() -> Option<Duration> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut times = [FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 }; 4];
    let [creation, exit, kernel, user] = &mut times;
    // SAFETY: the pseudo-handle of the current process is always valid, and the four
    // pointers are writable FILETIMEs.
    let ok = unsafe { GetProcessTimes(GetCurrentProcess(), creation, exit, kernel, user) };
    // FILETIME counts 100-nanosecond intervals.
    let ticks = |t: &FILETIME| u64::from(t.dwHighDateTime) << 32 | u64::from(t.dwLowDateTime);
    (ok != 0).then(|| Duration::from_nanos((ticks(&times[2]) + ticks(&times[3])) * 100))
}

/// The most memory the process has held in RAM at once (its peak working set), in MB.
#[cfg(windows)]
pub fn peak_rss_mb() -> Option<f64> {
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    let size = size_of::<PROCESS_MEMORY_COUNTERS>();
    let mut counters = std::mem::MaybeUninit::<PROCESS_MEMORY_COUNTERS>::zeroed();
    // SAFETY: the pseudo-handle is valid, and `counters` is writable for `size` bytes.
    let ok =
        unsafe { GetProcessMemoryInfo(GetCurrentProcess(), counters.as_mut_ptr(), size as u32) };
    // SAFETY: the call succeeded, so the struct is initialized (it was zeroed anyway).
    (ok != 0)
        .then(|| unsafe { counters.assume_init() }.PeakWorkingSetSize as f64 / (1024.0 * 1024.0))
}

/// Not measured on other targets.
#[cfg(not(any(unix, windows)))]
pub fn process_cpu_time() -> Option<Duration> {
    None
}

/// Not measured on other targets.
#[cfg(not(any(unix, windows)))]
pub fn peak_rss_mb() -> Option<f64> {
    None
}
