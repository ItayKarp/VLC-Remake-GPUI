//! Programs, titles and disc menus on the worker (S8.9). libVLC 3's API lists a disc's titles
//! and presses keys in its menus; a stream's programs are only the input's `program` variable,
//! read and set through the core ([`super::core_vars`]), as VLC's own Program menu does.

use std::ffi::{CStr, c_int, c_uint};
use std::time::Duration;

use super::api::{TitleDescription, borrowed_string};
use super::core_vars::{INPUT, Var};
use super::worker::Worker;
use crate::engine::backend::BackendEvent;
use crate::engine::display_text::clean_tag;
use crate::engine::navigation::{MenuKey, Navigation, Program, Programs, Title, Titles};

/// The input's variable holding the program playing, with every program as a choice.
const PROGRAM: &CStr = c"program";
/// `libvlc_title_menu`: the title is one of the disc's menus.
const TITLE_MENU: c_uint = 0x01;

impl Worker {
    /// Moves to another program or title, or presses a key in the menu showing.
    pub(super) fn navigate(&mut self, to: Navigation) {
        let api = &self.vlc.api;
        let player = self.handles.player;
        // SAFETY (each call): the player is live.
        match to {
            Navigation::Program(id) => {
                unsafe { self.vlc.set_child_var(player, INPUT, PROGRAM, Var::Integer(id)) };
                self.send_programs();
            }
            Navigation::Title(index) => {
                let Ok(index) = c_int::try_from(index) else { return };
                unsafe { (api.libvlc_media_player_set_title)(player, index) };
            }
            Navigation::Menu(key) => unsafe {
                (api.libvlc_media_player_navigate)(player, menu_key(key));
            },
        }
    }

    /// Sends the input's programs, and which one plays.
    pub(super) fn send_programs(&self) {
        // SAFETY: the player is live.
        let choices = unsafe { self.vlc.child_choices(self.handles.player, INPUT, PROGRAM) };
        let all = choices.all.into_iter().map(|(id, name)| Program { id, name }).collect();
        self.shared.send(BackendEvent::Programs(Programs { all, current: choices.current }));
    }

    /// Sends the media's titles, and which one plays.
    pub(super) fn send_titles(&self) {
        let api = &self.vlc.api;
        let player = self.handles.player;
        let mut list = std::ptr::null_mut();
        // SAFETY: the player is live; libVLC fills `list` with `count` entries we release.
        let count =
            unsafe { (api.libvlc_media_player_get_full_title_descriptions)(player, &raw mut list) };
        let mut all = Vec::new();
        if count > 0 && !list.is_null() {
            // SAFETY: libVLC returned `count` live entries, released right after reading.
            unsafe {
                let entries = std::slice::from_raw_parts(list, count as usize);
                all.extend(entries.iter().map(|&entry| title(&*entry)));
                (api.libvlc_title_descriptions_release)(list, count as c_uint);
            }
        }
        // SAFETY: the player is live.
        let current = unsafe { (api.libvlc_media_player_get_title)(player) };
        let current = usize::try_from(current).ok().filter(|&at| at < all.len());
        self.shared.send(BackendEvent::Titles(Titles { all, current }));
    }
}

/// What libVLC says of a title.
///
/// # Safety
///
/// `entry`'s name is NULL or a live string.
unsafe fn title(entry: &TitleDescription) -> Title {
    Title {
        // SAFETY: the caller's promise.
        name: clean_tag(unsafe { borrowed_string(entry.name) }),
        duration: u64::try_from(entry.duration)
            .ok()
            .filter(|&millis| millis > 0)
            .map(Duration::from_millis),
        is_menu: entry.flags & TITLE_MENU != 0,
    }
}

/// `libvlc_navigate_mode_t` for `key`.
fn menu_key(key: MenuKey) -> c_uint {
    match key {
        MenuKey::Activate => 0,
        MenuKey::Up => 1,
        MenuKey::Down => 2,
        MenuKey::Left => 3,
        MenuKey::Right => 4,
    }
}
