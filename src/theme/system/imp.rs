//! Windows: reads the preferences straight from `user32`, which is cheap and
//! synchronous.

use std::ffi::c_void;

use super::SystemPrefs;
use crate::theme::palette::SystemColors;

const SPI_GETHIGHCONTRAST: u32 = 0x0042;
const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;
const HCF_HIGHCONTRASTON: u32 = 0x1;

const COLOR_WINDOW: i32 = 5;
const COLOR_WINDOWTEXT: i32 = 8;
const COLOR_HIGHLIGHT: i32 = 13;
const COLOR_HIGHLIGHTTEXT: i32 = 14;
const COLOR_BTNFACE: i32 = 15;
const COLOR_HOTLIGHT: i32 = 26;

#[repr(C)]
struct HighContrastW {
    cb_size: u32,
    dw_flags: u32,
    lpsz_default_scheme: *mut u16,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, win_ini: u32) -> i32;
    fn GetSysColor(index: i32) -> u32;
}

/// Reads the current preferences from the system parameters.
pub fn read() -> SystemPrefs {
    SystemPrefs {
        reduce_motion: !client_area_animation(),
        high_contrast: high_contrast_on().then(system_colors),
    }
}

fn client_area_animation() -> bool {
    let mut enabled: i32 = 1;
    // SAFETY: SPI_GETCLIENTAREAANIMATION writes a BOOL to `pv`.
    let ok = unsafe {
        SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, (&raw mut enabled).cast(), 0)
    };
    ok == 0 || enabled != 0
}

fn high_contrast_on() -> bool {
    let mut hc = HighContrastW {
        cb_size: size_of::<HighContrastW>() as u32,
        dw_flags: 0,
        lpsz_default_scheme: std::ptr::null_mut(),
    };
    // SAFETY: SPI_GETHIGHCONTRAST fills a HIGHCONTRASTW whose cbSize is set.
    let ok =
        unsafe { SystemParametersInfoW(SPI_GETHIGHCONTRAST, hc.cb_size, (&raw mut hc).cast(), 0) };
    ok != 0 && hc.dw_flags & HCF_HIGHCONTRASTON != 0
}

fn system_colors() -> SystemColors {
    let color = |index| {
        // SAFETY: GetSysColor has no preconditions.
        let bgr = unsafe { GetSysColor(index) };
        let rgb = ((bgr & 0xFF) << 16) | (bgr & 0xFF00) | ((bgr >> 16) & 0xFF);
        gpui::rgb(rgb).into()
    };
    SystemColors {
        window: color(COLOR_WINDOW),
        window_text: color(COLOR_WINDOWTEXT),
        highlight: color(COLOR_HIGHLIGHT),
        highlight_text: color(COLOR_HIGHLIGHTTEXT),
        hotlight: color(COLOR_HOTLIGHT),
        button_face: color(COLOR_BTNFACE),
    }
}
