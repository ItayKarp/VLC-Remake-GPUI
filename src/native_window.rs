//! What GPUI has no call for, asked of the OS window behind a GPUI window (S5.4.1):
//! - keeping it above other windows ("Keep window above other windows"), and
//! - moving it, to fullscreen on another display or to keep a resized window on screen, and
//! - whether its display can show HDR ([`display_hdr`], S7.5.3).
//!
//! macOS sets the `NSWindow`'s level and frame; Windows uses `SetWindowPos`. Linux has neither
//! yet (X11 needs `_NET_WM_STATE_ABOVE`, and Wayland doesn't let a client place or raise
//! itself), so both report that they did nothing.

mod display_hdr;

use gpui::{App, Pixels, Point, Window, px};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

pub use display_hdr::{DisplayRange, display_range};

/// Keeps `window` above other windows, or not. Returns whether the OS could be asked.
pub fn set_keep_above(window: &Window, above: bool) -> bool {
    raw(window).is_some_and(|handle| platform::set_keep_above(handle, above))
}

/// Moves `window`'s top-left corner to `origin`, in GPUI's global coordinates. Returns whether
/// the OS could be asked.
pub fn move_to(window: &Window, origin: Point<Pixels>, cx: &App) -> bool {
    let primary_height =
        cx.primary_display().map_or(px(0.), |display| display.bounds().size.height);
    let scale = window.scale_factor();
    raw(window).is_some_and(|handle| {
        platform::move_to(handle, (f32::from(origin.x), f32::from(origin.y)), primary_height, scale)
    })
}

fn raw(window: &Window) -> Option<RawWindowHandle> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    Some(handle.as_raw())
}

#[cfg(target_os = "macos")]
mod platform {
    use gpui::Pixels;
    use objc2_app_kit::{NSFloatingWindowLevel, NSNormalWindowLevel, NSView, NSWindow};
    use objc2_foundation::NSPoint;
    use raw_window_handle::RawWindowHandle;

    pub fn set_keep_above(handle: RawWindowHandle, above: bool) -> bool {
        with_window(handle, |window| {
            window.setLevel(if above { NSFloatingWindowLevel } else { NSNormalWindowLevel });
        })
    }

    /// GPUI's global coordinates start at the main display's top left, y going down; AppKit's
    /// at its bottom left, y going up.
    pub fn move_to(handle: RawWindowHandle, (x, y): (f32, f32), primary: Pixels, _: f32) -> bool {
        let top_left = NSPoint::new(f64::from(x), f64::from(f32::from(primary) - y));
        with_window(handle, |window| window.setFrameTopLeftPoint(top_left))
    }

    pub(super) fn with_window(handle: RawWindowHandle, act: impl FnOnce(&NSWindow)) -> bool {
        let RawWindowHandle::AppKit(handle) = handle else { return false };
        // SAFETY: GPUI's AppKit handle points at the window's content view, alive as long as the
        // GPUI window that lent it, and this runs on the main thread inside a window update.
        let view = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let Some(window) = view.window() else { return false };
        act(&window);
        true
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;

    use gpui::Pixels;
    use raw_window_handle::RawWindowHandle;

    const HWND_TOPMOST: isize = -1;
    const HWND_NOTOPMOST: isize = -2;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_NOACTIVATE: u32 = 0x0010;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWindowPos(
            window: *mut c_void,
            after: isize,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            flags: u32,
        ) -> i32;
    }

    pub fn set_keep_above(handle: RawWindowHandle, above: bool) -> bool {
        let after = if above { HWND_TOPMOST } else { HWND_NOTOPMOST };
        set_window_pos(handle, after, (0, 0), SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
    }

    /// GPUI's coordinates on Windows are logical; `SetWindowPos` takes physical pixels.
    pub fn move_to(handle: RawWindowHandle, (x, y): (f32, f32), _: Pixels, scale: f32) -> bool {
        let at = ((x * scale).round() as i32, (y * scale).round() as i32);
        set_window_pos(handle, 0, at, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
    }

    fn set_window_pos(
        handle: RawWindowHandle,
        after: isize,
        (x, y): (i32, i32),
        flags: u32,
    ) -> bool {
        let RawWindowHandle::Win32(handle) = handle else { return false };
        let window = handle.hwnd.get() as *mut c_void;
        // SAFETY: a valid HWND from GPUI, owned by this thread; no size or z-order change is
        // asked for beyond the flags.
        unsafe { SetWindowPos(window, after, x, y, 0, 0, flags) != 0 }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use gpui::Pixels;
    use raw_window_handle::RawWindowHandle;

    pub fn set_keep_above(_: RawWindowHandle, _: bool) -> bool {
        false
    }

    pub fn move_to(_: RawWindowHandle, _: (f32, f32), _: Pixels, _: f32) -> bool {
        false
    }
}
