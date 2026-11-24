//! The buttons Customize Player adds that no command stands behind (S8.7.6):
//!
//! - **Favorite** marks the playing file a favorite (S4.8.3), lit while it is one; it waits
//!   for the library to know the file, and streams can't be favorites.
//! - **HDR** switches the playing video's tone mapping between Auto and Off for this session,
//!   as the settings panel's HDR row does, lit while the video is tone-mapped.
//! - **Zoom** steps the playing video's zoom through Settings' choices for this session, lit
//!   while it isn't Fit to Window.
//! - **Right Sidebar** shows or hides the panel on the right, the queue panel, on the tab it
//!   last showed.
//!
//! HDR and Zoom say over the video what they changed to.

use gpui::{App, Context};

use super::buttons::{ButtonItem, Press};
use crate::assets::icons;
use crate::db::activity::Favorite;
use crate::effects;
use crate::library::favorites;
use crate::main_window::MainWindow;
use crate::player_layout::Item;
use crate::settings::registry::{self, Choice, Control, Value};

/// The settings HDR and Zoom change.
const HDR: &str = "video.hdr";
const ZOOM: &str = "video.zoom";
/// HDR's choices the button switches between.
const HDR_AUTO: &str = "auto";
const HDR_OFF: &str = "off";

impl MainWindow {
    /// The button `item` is, if it is one of these.
    pub(super) fn extra_button(&self, item: Item, cx: &App) -> Option<ButtonItem> {
        let video = self.loaded_player(cx).filter(|player| player.read(cx).video_size().is_some());
        let button = |icon, label, press, enabled, selected| {
            Some(ButtonItem { icon, label, press, enabled, selected })
        };
        match item {
            Item::Favorite => {
                let marked = self.playing_id(cx).map(Favorite::Media);
                let on = marked.is_some_and(|marked| favorites::all(&[marked], cx));
                let label = if on { tr!("Remove from Favorites") } else { tr!("Add to Favorites") };
                button(icons::FAVORITE, label, Press::Favorite, marked.is_some(), on)
            }
            Item::Hdr => {
                let mapped = video.as_ref().is_some_and(|v| v.read(cx).hdr().tone_map.is_some());
                button(icons::HDR, tr!("HDR"), Press::Hdr, video.is_some(), mapped)
            }
            Item::Zoom => {
                let zoomed = choice(ZOOM, cx).is_some_and(|(index, _)| index > 0);
                button(icons::ZOOM, tr!("Zoom"), Press::Zoom, video.is_some(), zoomed)
            }
            Item::RightSidebar => {
                let open = self.queue_panel.is_open();
                button(icons::SIDEBAR_RIGHT, tr!("Right sidebar"), Press::RightSidebar, true, open)
            }
            _ => None,
        }
    }

    /// Marks the playing file a favorite, or stops it being one.
    pub(super) fn toggle_playing_favorite(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.playing_id(cx) else { return };
        let marked = Favorite::Media(id);
        let on = favorites::all(&[marked], cx);
        favorites::mark(vec![marked], !on, cx);
        cx.notify();
    }

    /// Switches the playing video's HDR between Auto and Off.
    pub(super) fn toggle_session_hdr(&mut self, cx: &mut Context<Self>) {
        let off = choice(HDR, cx).is_some_and(|(_, current)| current.id == HDR_OFF);
        let to = if off { HDR_AUTO } else { HDR_OFF };
        effects::set(cx, HDR, Value::Choice(to));
        if let Some((_, chosen)) = choice(HDR, cx) {
            self.show_osd(tr!("HDR: {choice}", choice = crate::i18n::text(chosen.label)), cx);
        }
    }

    /// Steps the playing video's zoom to its next choice, after the last back to the first.
    pub(super) fn step_session_zoom(&mut self, cx: &mut Context<Self>) {
        let (Some(choices), Some((index, _))) = (choices(ZOOM), choice(ZOOM, cx)) else { return };
        let next = &choices[(index + 1) % choices.len()];
        effects::set(cx, ZOOM, Value::Choice(next.id));
        self.show_osd(tr!("Zoom: {choice}", choice = crate::i18n::text(next.label)), cx);
    }
}

/// The choices of the setting `key`.
fn choices(key: &str) -> Option<&'static [Choice]> {
    match registry::find(key)?.control {
        Control::Dropdown(choices) | Control::Segmented(choices) => Some(choices),
        _ => None,
    }
}

/// Which of `key`'s choices is in effect, and where it is among them.
fn choice(key: &str, cx: &App) -> Option<(usize, &'static Choice)> {
    let current = registry::find(key)?.value(&effects::effective(cx));
    let choices = choices(key)?;
    choices.iter().enumerate().find(|(_, choice)| current == Value::Choice(choice.id))
}

#[cfg(test)]
mod tests;
