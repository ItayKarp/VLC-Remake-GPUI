//! Which display a new window opens on (S0.2.8).

use gpui::{App, DisplayId};

/// The display with the mouse cursor, if the platform can tell. GPUI has no
/// app-level cursor query, so this asks the OS directly.
#[cfg(windows)]
pub fn cursor_display(cx: &App) -> Option<DisplayId> {
    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }
    const MONITOR_DEFAULTTONULL: u32 = 0;
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetCursorPos(point: *mut Point) -> i32;
        fn MonitorFromPoint(point: Point, flags: u32) -> *mut core::ffi::c_void;
    }

    let mut point = Point { x: 0, y: 0 };
    // SAFETY: plain Win32 calls; `point` is a valid out-pointer.
    let monitor = unsafe {
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
        MonitorFromPoint(point, MONITOR_DEFAULTTONULL)
    };
    if monitor.is_null() {
        return None;
    }
    // GPUI's Windows backend uses the HMONITOR as the display ID.
    let id = DisplayId::new(monitor as u64);
    cx.find_display(id).map(|display| display.id())
}

/// macOS and Linux: not implemented yet, so new windows open on the primary display
/// (or wherever the compositor places them on Wayland).
#[cfg(not(windows))]
pub fn cursor_display(_cx: &App) -> Option<DisplayId> {
    None
}

/// The display of the active window, else the one with the cursor.
pub fn preferred_display(cx: &mut App) -> Option<DisplayId> {
    let from_window = cx.active_window().and_then(|handle| {
        handle
            .update(cx, |_, window, cx| window.display(cx).map(|d| d.id()))
            .ok()
            .flatten()
    });
    from_window.or_else(|| cursor_display(cx))
}
