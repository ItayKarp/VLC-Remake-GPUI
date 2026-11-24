//! Putting a window behind the desktop icons, for "Play video as desktop wallpaper" (S8.16.3).
//! Only Windows has such a place: Explorer draws the wallpaper in a `WorkerW` window behind the
//! icons' `SHELLDLL_DefView`, and asking `Progman` for message 0x052C makes it create one. The
//! window becomes that `WorkerW`'s child, covering the main display. Elsewhere it does nothing.

use gpui::Window;

use super::raw;

/// Makes `window` the desktop's wallpaper, unless it already is. Returns whether it is now.
pub fn keep_as_wallpaper(window: &Window) -> bool {
    raw(window).is_some_and(platform::keep_as_wallpaper)
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ptr::{null, null_mut};

    use raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, TRUE};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, FindWindowW, GWL_STYLE, GetClassNameW, GetParent,
        GetSystemMetrics, GetWindowLongPtrW, SM_CXSCREEN, SM_CYSCREEN, SM_XVIRTUALSCREEN,
        SM_YVIRTUALSCREEN, SMTO_NORMAL, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOZORDER,
        SWP_SHOWWINDOW, SendMessageTimeoutW, SetParent, SetWindowLongPtrW, SetWindowPos,
        WS_CAPTION, WS_CHILD, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
    };

    /// The undocumented message that makes `Progman` create the wallpaper's `WorkerW`.
    const SPAWN_WORKER: u32 = 0x052C;
    /// How long Explorer gets to answer it, in milliseconds.
    const SPAWN_TIMEOUT_MS: u32 = 1000;

    pub fn keep_as_wallpaper(handle: RawWindowHandle) -> bool {
        let RawWindowHandle::Win32(handle) = handle else { return false };
        let window = handle.hwnd.get() as HWND;
        // SAFETY: a valid HWND from GPUI, on the thread that owns it.
        if is_worker(unsafe { GetParent(window) }) {
            return true;
        }
        let Some(worker) = find_worker() else { return false };
        // SAFETY: as above; the worker is a live window Explorer owns.
        unsafe { attach(window, worker) }
    }

    /// Makes `window` a borderless child of `worker`, over the main display.
    unsafe fn attach(window: HWND, worker: HWND) -> bool {
        let frame =
            WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MAXIMIZEBOX | WS_MINIMIZEBOX;
        // SAFETY: the caller's windows; style bits are plain integers.
        unsafe {
            let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
            SetWindowLongPtrW(window, GWL_STYLE, ((style & !frame) | WS_CHILD) as isize);
            if SetParent(window, worker).is_null() {
                return false;
            }
            // The worker spans every display from the virtual screen's top left; the main
            // display starts at the screen's origin.
            let (left, top) =
                (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let (width, height) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
            let flags = SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW;
            SetWindowPos(window, null_mut(), -left, -top, width, height, flags) != 0
        }
    }

    /// The `WorkerW` behind the desktop icons, asking Explorer to make it first.
    fn find_worker() -> Option<HWND> {
        let progman_class = wide("Progman");
        let worker_class = wide("WorkerW");
        // SAFETY: plain Win32 lookups with null-terminated names that outlive the calls.
        unsafe {
            let progman = FindWindowW(progman_class.as_ptr(), null());
            if progman.is_null() {
                return None;
            }
            let mut answer = 0;
            SendMessageTimeoutW(
                progman,
                SPAWN_WORKER,
                0xD,
                1,
                SMTO_NORMAL,
                SPAWN_TIMEOUT_MS,
                &raw mut answer,
            );
            // Windows 11 24H2 and later keep it inside Progman.
            let inside = FindWindowExW(progman, null_mut(), worker_class.as_ptr(), null());
            if !inside.is_null() {
                return Some(inside);
            }
            // Before that, it is the top-level WorkerW after the one holding the icons.
            let mut found: HWND = null_mut();
            EnumWindows(Some(find_icons_sibling), &raw mut found as LPARAM);
            (!found.is_null()).then_some(found)
        }
    }

    /// For `EnumWindows`: the `WorkerW` after the window holding `SHELLDLL_DefView`.
    unsafe extern "system" fn find_icons_sibling(window: HWND, found: LPARAM) -> i32 {
        let icons_class = wide("SHELLDLL_DefView");
        let worker_class = wide("WorkerW");
        // SAFETY: `found` points at the caller's HWND for the whole enumeration.
        unsafe {
            let icons = FindWindowExW(window, null_mut(), icons_class.as_ptr(), null());
            if icons.is_null() {
                return TRUE;
            }
            *(found as *mut HWND) =
                FindWindowExW(null_mut(), window, worker_class.as_ptr(), null());
        }
        0
    }

    /// Whether `window` is a `WorkerW`.
    fn is_worker(window: HWND) -> bool {
        if window.is_null() {
            return false;
        }
        let mut name = [0_u16; 16];
        // SAFETY: a buffer of the length passed.
        let length = unsafe { GetClassNameW(window, name.as_mut_ptr(), name.len() as i32) };
        let name = name.get(..usize::try_from(length).unwrap_or(0)).unwrap_or_default();
        String::from_utf16_lossy(name) == "WorkerW"
    }

    /// `text` as a null-terminated UTF-16 string.
    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use raw_window_handle::RawWindowHandle;

    pub fn keep_as_wallpaper(_: RawWindowHandle) -> bool {
        false
    }
}
