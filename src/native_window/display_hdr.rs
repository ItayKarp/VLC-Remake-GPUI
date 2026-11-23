//! Whether the display a window is on can show HDR (S7.5.3).
//!
//! macOS says so per screen (`NSScreen`'s potential extended dynamic range above 1). Windows and
//! Linux aren't asked yet (DXGI's output color space; Wayland's color management), so their
//! displays count as unknown. Nothing shows HDR through GPUI's 8-bit surfaces yet, so this only
//! tells the user why HDR video is tone-mapped even on such a display; the main window re-reads
//! it whenever it moves, so it follows the window from one display to another.

use gpui::Window;

/// What a display can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayRange {
    /// HDR (macOS's extended dynamic range).
    Hdr,
    /// SDR only.
    #[cfg_attr(
        not(target_os = "macos"),
        allow(
            dead_code,
            reason = "only macOS asks its screens yet; elsewhere displays are Unknown"
        )
    )]
    Sdr,
    /// The OS wasn't asked.
    Unknown,
}

/// What the display `window` is on can show.
pub fn display_range(window: &Window) -> DisplayRange {
    super::raw(window).map_or(DisplayRange::Unknown, platform::display_range)
}

#[cfg(target_os = "macos")]
mod platform {
    use raw_window_handle::RawWindowHandle;

    use super::DisplayRange;

    /// The most a component may be above 1 for the screen to count as SDR.
    const SDR_HEADROOM: f64 = 1.0;

    pub fn display_range(handle: RawWindowHandle) -> DisplayRange {
        let mut range = DisplayRange::Unknown;
        super::super::platform::with_window(handle, |window| {
            let Some(screen) = window.screen() else { return };
            let headroom = screen.maximumPotentialExtendedDynamicRangeColorComponentValue();
            range = if headroom > SDR_HEADROOM { DisplayRange::Hdr } else { DisplayRange::Sdr };
        });
        range
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    use raw_window_handle::RawWindowHandle;

    use super::DisplayRange;

    pub fn display_range(_: RawWindowHandle) -> DisplayRange {
        DisplayRange::Unknown
    }
}
