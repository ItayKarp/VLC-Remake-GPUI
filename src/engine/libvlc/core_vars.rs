//! The player's variables, through libVLC's core (`libvlccore`): `var_Create`, `var_SetChecked`
//! and `var_GetChecked`, the calls libVLC's own `libvlc_video_set_adjust_*` and
//! `libvlc_video_set_deinterlace` make; and its video outputs' (`vlc_list_children`), which a
//! filter's settings reach while it runs, as libVLC's own calls reach them. And the description
//! of the input playing (`input_GetItem`, `input_item_GetInfo`), which says what libVLC's API
//! doesn't, such as a video's colors (`stream_colour.rs`).
//!
//! libVLC 3 opens video outputs under the player, so they take their filters (`video-filter`,
//! `transform-type`, `deinterlace`, `deinterlace-mode`) from its variables when they open; the
//! media's options never reach them. Its API sets only some of those variables, and
//! deinterlacing only on or off, never automatic.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::slice;

use anyhow::{Context as _, Result};
use libloading::Library;

use super::api::{LibVlc, MediaPlayer};

/// `VLC_VAR_BOOL`, `VLC_VAR_INTEGER`, `VLC_VAR_STRING` and `VLC_VAR_FLOAT`.
const VAR_BOOL: c_int = 0x0020;
const VAR_INTEGER: c_int = 0x0030;
const VAR_STRING: c_int = 0x0040;
const VAR_FLOAT: c_int = 0x0050;
/// A video output's and an input's object types, the first member of every libVLC 3 object.
const VIDEO_OUTPUT: &CStr = c"video output";
pub(super) const INPUT: &CStr = c"input";
/// `VLC_SUCCESS`.
const SUCCESS: c_int = 0;
/// `VLC_VAR_GETCHOICES`: `var_Change`'s action listing a variable's choices and their names.
const GET_CHOICES: c_int = 0x0024;

/// The choices of an integer variable, such as the input's `program`, and the one it holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Choices {
    /// Each choice's value and name, in libVLC's order.
    pub all: Vec<(i64, Option<String>)>,
    /// The value the variable holds.
    pub current: Option<i64>,
}

/// `vlc_value_t`, reduced to the members read and written here; eight bytes, like the C union.
#[repr(C)]
#[derive(Clone, Copy)]
union VlcValue {
    boolean: bool,
    integer: i64,
    float: f32,
    string: *const c_char,
    address: *mut c_void,
}

/// `vlc_list_t`: the objects `vlc_list_children` holds.
#[repr(C)]
struct VlcList {
    _kind: c_int,
    count: c_int,
    values: *const VlcValue,
}

type VarCreate = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> c_int;
type VarSet = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, VlcValue) -> c_int;
type VarGet = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut VlcValue) -> c_int;
type ListChildren = unsafe extern "C" fn(*mut c_void) -> *mut VlcList;
type ListRelease = unsafe extern "C" fn(*mut VlcList);
type InputGetItem = unsafe extern "C" fn(*mut c_void) -> *mut c_void;
type ItemGetInfo = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> *mut c_char;
type VarChange =
    unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut VlcValue, *mut VlcValue) -> c_int;
type VarFreeList = unsafe extern "C" fn(*mut VlcValue, *mut VlcValue);

/// The core's variable calls.
pub struct CoreVars {
    create: VarCreate,
    set: VarSet,
    get: VarGet,
    list_children: ListChildren,
    list_release: ListRelease,
    /// `input_GetItem`: an input's item, which it holds while it lives.
    pub(super) input_item: InputGetItem,
    /// `input_item_GetInfo`: a line of an item's description, a copy the caller frees.
    pub(super) item_info: ItemGetInfo,
    /// `var_Change`: a variable's choices, among other things.
    change: VarChange,
    /// `var_FreeList`: frees the choices `var_Change` handed over.
    free_list: VarFreeList,
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
                list_children: *core
                    .get::<ListChildren>(b"vlc_list_children\0")
                    .context("vlc_list_children")?,
                list_release: *core
                    .get::<ListRelease>(b"vlc_list_release\0")
                    .context("vlc_list_release")?,
                input_item: *core
                    .get::<InputGetItem>(b"input_GetItem\0")
                    .context("input_GetItem")?,
                item_info: *core
                    .get::<ItemGetInfo>(b"input_item_GetInfo\0")
                    .context("input_item_GetInfo")?,
                change: *core.get::<VarChange>(b"var_Change\0").context("var_Change")?,
                free_list: *core.get::<VarFreeList>(b"var_FreeList\0").context("var_FreeList")?,
            })
        }
    }
}

/// The value of one of the player's variables.
#[derive(Clone, Copy, Debug)]
pub enum Var<'a> {
    /// On or off.
    Bool(bool),
    /// A number.
    Integer(i64),
    /// A fraction.
    Float(f32),
    /// Text.
    String(&'a CStr),
}

impl Var<'_> {
    /// libVLC's type for it, and its value.
    fn raw(self) -> (c_int, VlcValue) {
        match self {
            Self::Bool(boolean) => (VAR_BOOL, VlcValue { boolean }),
            Self::Integer(integer) => (VAR_INTEGER, VlcValue { integer }),
            Self::Float(float) => (VAR_FLOAT, VlcValue { float }),
            Self::String(text) => (VAR_STRING, VlcValue { string: text.as_ptr() }),
        }
    }
}

impl LibVlc {
    /// Sets the player's variable `name`, making it first if libVLC hasn't; logs a failure.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn set_player_var(&self, player: *mut MediaPlayer, name: &CStr, to: Var) {
        let (kind, value) = to.raw();
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

    /// Sets the variable `name` of each of the player's video outputs, where it is: a running
    /// filter's setting, which reaches the filter at once. An output without it is left alone.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn set_output_var(&self, player: *mut MediaPlayer, name: &CStr, to: Var) {
        let (kind, value) = to.raw();
        // SAFETY: the caller's promise; each output is live during the call.
        unsafe {
            self.for_each_child(player, VIDEO_OUTPUT, |output| {
                (self.core.set)(output, name.as_ptr(), kind, value);
            });
        }
    }

    /// Sets the variable `name` of each of the player's children of type `kind`, making it
    /// where a child hasn't: a child that made it already keeps it rather than inheriting the
    /// player's again (an audio output's `equalizer-2pass`).
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn set_child_var(
        &self,
        player: *mut MediaPlayer,
        kind: &CStr,
        name: &CStr,
        to: Var,
    ) {
        let (var_kind, value) = to.raw();
        // SAFETY: the caller's promise; each child is live during the call, and a second
        // `var_Create` of a variable only counts another reference to it.
        unsafe {
            self.for_each_child(player, kind, |child| {
                (self.core.create)(child, name.as_ptr(), var_kind);
                (self.core.set)(child, name.as_ptr(), var_kind, value);
            });
        }
    }

    /// The boolean variable `name` of the player's first child of type `kind`; `None` without
    /// such a child or variable: whether the input is recording (S8.6), and the smoke tests'.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn child_bool(
        &self,
        player: *mut MediaPlayer,
        kind: &CStr,
        name: &CStr,
    ) -> Option<bool> {
        let mut found = None;
        // SAFETY: the caller's promise; each child is live during the call.
        unsafe {
            self.for_each_child(player, kind, |child| {
                let mut value = VlcValue { integer: 0 };
                if found.is_none()
                    && (self.core.get)(child, name.as_ptr(), VAR_BOOL, &raw mut value) == SUCCESS
                {
                    found = Some(value.boolean);
                }
            });
        }
        found
    }

    /// The text variable `name` of the player's first child of type `kind`; `None` without
    /// such a child or variable. For the smoke tests.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    #[cfg(test)]
    pub(super) unsafe fn child_string(
        &self,
        player: *mut MediaPlayer,
        kind: &CStr,
        name: &CStr,
    ) -> Option<String> {
        let mut found = None;
        // SAFETY: the caller's promise; each child is live during the call, and a string the
        // core hands over is freed by `take_string`.
        unsafe {
            self.for_each_child(player, kind, |child| {
                let mut value = VlcValue { string: std::ptr::null() };
                if found.is_none()
                    && (self.core.get)(child, name.as_ptr(), VAR_STRING, &raw mut value) == SUCCESS
                {
                    found = self.take_string(value.string.cast_mut());
                }
            });
        }
        found
    }

    /// Calls `f` with each of the player's child objects of type `kind`, held meanwhile.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn for_each_child(
        &self,
        player: *mut MediaPlayer,
        kind: &CStr,
        mut f: impl FnMut(*mut c_void),
    ) {
        // SAFETY: the caller's promise; the list holds each child until it is released.
        unsafe {
            let list = (self.core.list_children)(player.cast());
            if list.is_null() {
                return;
            }
            let count = usize::try_from((*list).count).unwrap_or_default();
            let children =
                if count == 0 { &[][..] } else { slice::from_raw_parts((*list).values, count) };
            for child in children {
                let object = child.address;
                if !object.is_null() && is_of_type(object, kind) {
                    f(object);
                }
            }
            (self.core.list_release)(list);
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

impl LibVlc {
    /// The choices of the integer variable `name` of the player's first child of type `kind`,
    /// and the one it holds; empty without such a child or variable.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`, which is a libVLC object.
    pub(super) unsafe fn child_choices(
        &self,
        player: *mut MediaPlayer,
        kind: &CStr,
        name: &CStr,
    ) -> Choices {
        let mut choices = None;
        // SAFETY: the caller's promise; each child is live during the call.
        unsafe {
            self.for_each_child(player, kind, |child| {
                if choices.is_none() {
                    choices = Some(self.choices_of(child, name));
                }
            });
        }
        choices.unwrap_or_default()
    }

    /// The choices of `object`'s integer variable `name`, and the one it holds.
    ///
    /// # Safety
    ///
    /// `object` is a live libVLC object.
    unsafe fn choices_of(&self, object: *mut c_void, name: &CStr) -> Choices {
        let mut values = VlcValue { integer: 0 };
        let mut names = VlcValue { integer: 0 };
        let mut current = VlcValue { integer: 0 };
        // SAFETY: the caller's promise. On success the core hands over two lists, one of
        // values and one of their names (strings it owns), which `var_FreeList` frees.
        unsafe {
            let listed = (self.core.change)(
                object,
                name.as_ptr(),
                GET_CHOICES,
                &raw mut values,
                &raw mut names,
            );
            if listed != SUCCESS {
                return Choices::default();
            }
            let all = list_values(values.address.cast())
                .iter()
                .zip(list_values(names.address.cast()))
                .map(|(value, name)| (value.integer, super::api::borrowed_string(name.string)))
                .collect();
            (self.core.free_list)(&raw mut values, &raw mut names);
            let held = (self.core.get)(object, name.as_ptr(), VAR_INTEGER, &raw mut current);
            Choices { all, current: (held == SUCCESS).then_some(current.integer) }
        }
    }
}

/// The values a `vlc_list_t` holds.
///
/// # Safety
///
/// `list` is NULL or a live `vlc_list_t`, which outlives the slice.
unsafe fn list_values<'a>(list: *const VlcList) -> &'a [VlcValue] {
    // SAFETY: the caller's promise.
    unsafe {
        let Some(list) = list.as_ref() else { return &[] };
        let count = usize::try_from(list.count).unwrap_or_default();
        if count == 0 || list.values.is_null() {
            &[]
        } else {
            slice::from_raw_parts(list.values, count)
        }
    }
}

/// Whether `object` is of type `kind`, by the type name libVLC 3 puts first in every object.
///
/// # Safety
///
/// `object` is a live libVLC object.
unsafe fn is_of_type(object: *mut c_void, kind: &CStr) -> bool {
    // SAFETY: the caller's promise; `object_type` is a constant string libVLC never frees.
    unsafe {
        let name = *object.cast::<*const c_char>();
        !name.is_null() && CStr::from_ptr(name) == kind
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
