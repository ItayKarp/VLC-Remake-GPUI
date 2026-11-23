//! The player's variables, through libVLC's core (`libvlccore`): `var_Create`, `var_SetChecked`
//! and `var_GetChecked`, the calls libVLC's own `libvlc_video_set_adjust_*` and
//! `libvlc_video_set_deinterlace` make.
//!
//! libVLC 3 opens video outputs under the player, so they take their filters (`video-filter`,
//! `transform-type`, `deinterlace`, `deinterlace-mode`) from its variables when they open; the
//! media's options never reach them. Its API sets only some of those variables, and
//! deinterlacing only on or off, never automatic.

use std::ffi::{CStr, c_char, c_int, c_void};

use anyhow::{Context as _, Result};
use libloading::Library;

use super::api::{LibVlc, MediaPlayer};

/// `VLC_VAR_INTEGER` and `VLC_VAR_STRING`.
const VAR_INTEGER: c_int = 0x0030;
const VAR_STRING: c_int = 0x0040;
/// `VLC_SUCCESS`.
const SUCCESS: c_int = 0;

/// `vlc_value_t`, reduced to the members read and written here; eight bytes, like the C union.
#[repr(C)]
#[derive(Clone, Copy)]
union VlcValue {
    integer: i64,
    string: *const c_char,
}

type VarCreate = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type VarSet = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, VlcValue) -> c_int;
type VarGet = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut VlcValue) -> c_int;

/// The core's variable calls.
pub struct CoreVars {
    create: VarCreate,
    set: VarSet,
    get: VarGet,
}

impl CoreVars {
    /// Finds the calls in the loaded `libvlccore`.
    pub(super) fn resolve(core: &Library) -> Result<Self> {
        // SAFETY: each symbol has the libVLC 3 core signature declared above.
        unsafe {
            Ok(Self {
                create: *core.get::<VarCreate>(b"var_Create\0").context("var_Create")?,
                set: *core.get::<VarSet>(b"var_SetChecked\0").context("var_SetChecked")?,
                get: *core.get::<VarGet>(b"var_GetChecked\0").context("var_GetChecked")?,
            })
        }
    }
}

/// The value of one of the player's variables.
#[derive(Clone, Copy, Debug)]
pub enum Var<'a> {
    /// A number.
    Integer(i64),
    /// Text.
    String(&'a CStr),
}

impl LibVlc {
    /// Sets the player's variable `name`, making it first if libVLC hasn't; logs a failure.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn set_player_var(&self, player: *mut MediaPlayer, name: &CStr, to: Var) {
        let (kind, value) = match to {
            Var::Integer(integer) => (VAR_INTEGER, VlcValue { integer }),
            Var::String(text) => (VAR_STRING, VlcValue { string: text.as_ptr() }),
        };
        let object = player.cast();
        // SAFETY: the caller's promise; the core copies a string value.
        let done = unsafe {
            (self.core.create)(object, name.as_ptr(), kind) == SUCCESS
                && (self.core.set)(object, name.as_ptr(), kind, value) == SUCCESS
        };
        if !done {
            tracing::warn!("libvlc: couldn't set the player's {name:?}");
        }
    }

    /// The player's text variable `name`; `None` when it is unset or empty.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`.
    pub(super) unsafe fn player_string(
        &self,
        player: *mut MediaPlayer,
        name: &CStr,
    ) -> Option<String> {
        let mut value = VlcValue { string: std::ptr::null() };
        // SAFETY: the caller's promise; on success the core hands over a string the caller
        // frees, which `take_string` does.
        unsafe {
            if (self.core.get)(player.cast(), name.as_ptr(), VAR_STRING, &raw mut value) != SUCCESS
            {
                return None;
            }
            self.take_string(value.string.cast_mut())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_is_as_large_as_libvlcs() {
        assert_eq!(size_of::<VlcValue>(), 8);
    }
}
