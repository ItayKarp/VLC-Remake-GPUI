//! The Picture in Picture window's OS calls (S8.5): floating above other windows, also over
//! another app's fullscreen space where the OS allows it, without window buttons, and resizing
//! with the video's aspect ratio kept.
//!
//! macOS does all of it on the `NSWindow`. Windows only keeps the window on top; the window's
//! view keeps the aspect ratio itself after each resize. Linux does neither (Wayland doesn't let
//! a client raise itself), so the window floats only if the window manager is told to.

use gpui::Window;

/// Makes `window` float above others, on every space, without the OS window buttons. Returns
/// whether the OS could be asked to keep it on top.
pub fn make_floating(window: &Window) -> bool {
    super::raw(window).is_some_and(platform::make_floating)
}

/// Keeps `window`'s content at `aspect` (width over height) while the user resizes it. Returns
/// whether the OS does it; when not, the caller corrects the size after each resize.
pub fn lock_aspect_ratio(window: &Window, aspect: (f32, f32)) -> bool {
    super::raw(window).is_some_and(|handle| platform::lock_aspect_ratio(handle, aspect))
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_app_kit::{NSFloatingWindowLevel, NSWindowButton, NSWindowCollectionBehavior};
    use objc2_foundation::NSSize;
    use raw_window_handle::RawWindowHandle;

    use super::super::platform::with_window;

    /// The traffic lights a borderless-looking window hides.
    const BUTTONS: [NSWindowButton; 3] = [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ];

    pub fn make_floating(handle: RawWindowHandle) -> bool {
        with_window(handle, |window| {
            window.setLevel(NSFloatingWindowLevel);
            // On every space, and over another app's fullscreen space.
            window.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary,
            );
            let buttons = BUTTONS.into_iter().filter_map(|kind| window.standardWindowButton(kind));
            buttons.for_each(|button| button.setHidden(true));
        })
    }

    pub fn lock_aspect_ratio(handle: RawWindowHandle, (width, height): (f32, f32)) -> bool {
        let aspect = NSSize::new(f64::from(width), f64::from(height));
        with_window(handle, |window| window.setContentAspectRatio(aspect))
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use raw_window_handle::RawWindowHandle;

    pub fn make_floating(handle: RawWindowHandle) -> bool {
        super::super::platform::set_keep_above(handle, true)
    }

    pub fn lock_aspect_ratio(_: RawWindowHandle, _: (f32, f32)) -> bool {
        false
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod platform {
    use raw_window_handle::RawWindowHandle;

    pub fn make_floating(_: RawWindowHandle) -> bool {
        false
    }

    pub fn lock_aspect_ratio(_: RawWindowHandle, _: (f32, f32)) -> bool {
        false
    }
}
