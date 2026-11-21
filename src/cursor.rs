//! Hides the mouse pointer until it moves, as the player's controls hide (S2.8.1). GPUI hides it
//! only while typing, so this asks the OS directly:
//! - macOS: `NSCursor.setHiddenUntilMouseMoves`.
//! - Windows: no cursor until the next `WM_SETCURSOR`, which GPUI answers on the next move.
//! - Linux: not supported yet (X11 and Wayland each need their own invisible cursor); the
//!   pointer stays.

/// Hides the pointer until the mouse next moves. Call it only while the pointer is over the
/// active window: the OS hides it wherever it is.
pub fn hide_until_moved() {
    platform::hide_until_moved();
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::NSCursor;

    pub fn hide_until_moved() {
        NSCursor::setHiddenUntilMouseMoves(true);
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ffi::c_void;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetCursor(cursor: *mut c_void) -> *mut c_void;
    }

    pub fn hide_until_moved() {
        // SAFETY: a null cursor is documented to remove the cursor from the screen.
        unsafe { SetCursor(std::ptr::null_mut()) };
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    pub fn hide_until_moved() {}
}
