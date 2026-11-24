//! Hiding a window without closing it, for "Keep playing in the tray when the window closes"
//! (S8.16.1): GPUI can only minimize. macOS orders the `NSWindow` out and back in; Windows hides
//! and shows the `HWND`. Linux has no call for it (Wayland doesn't let a client hide itself), so
//! it reports that it did nothing.

use gpui::Window;

use super::raw;

/// Hides `window`, or shows it again. Returns whether the OS could be asked.
pub fn set_hidden(window: &Window, hidden: bool) -> bool {
    raw(window).is_some_and(|handle| platform::set_hidden(handle, hidden))
}

#[cfg(target_os = "macos")]
mod platform {
    use raw_window_handle::RawWindowHandle;

    pub fn set_hidden(handle: RawWindowHandle, hidden: bool) -> bool {
        super::super::platform::with_window(handle, |window| {
            if hidden {
                window.orderOut(None);
            } else {
                window.makeKeyAndOrderFront(None);
            }
        })
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;

    use raw_window_handle::RawWindowHandle;

    const SW_HIDE: i32 = 0;
    const SW_SHOW: i32 = 5;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn ShowWindow(window: *mut c_void, command: i32) -> i32;
        fn SetForegroundWindow(window: *mut c_void) -> i32;
    }

    pub fn set_hidden(handle: RawWindowHandle, hidden: bool) -> bool {
        let RawWindowHandle::Win32(handle) = handle else { return false };
        let window = handle.hwnd.get() as *mut c_void;
        // SAFETY: a valid HWND from GPUI, owned by this thread.
        unsafe {
            ShowWindow(window, if hidden { SW_HIDE } else { SW_SHOW });
            if !hidden {
                SetForegroundWindow(window);
            }
        }
        true
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use raw_window_handle::RawWindowHandle;

    pub fn set_hidden(_: RawWindowHandle, _: bool) -> bool {
        false
    }
}
