//! The bar items that aren't icon buttons or playback controls: the names (the title, the
//! file's and its folder's) and the title bar's own items when they are moved (the `…` menu,
//! the sidebar toggle and the folder chip, S8.7.3).

use gpui::{AnyElement, App, Context, SharedString, div, prelude::*, px};

use super::items::Place;
use super::layout::NAME_WIDTH;
use crate::app::media::Media;
use crate::library;
use crate::main_window::{MainWindow, customize};
use crate::player_layout::{Item, Unit};
use crate::theme::Palette;

/// The names' text size, the title bar's.
const NAME_TEXT: f32 = 13.;

impl MainWindow {
    /// Draws `item`, one of the names or the title bar's own items, in the bar `place`.
    pub(super) fn render_named_item(
        &self,
        item: Item,
        place: Place,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // In edit mode an item that shows nothing yet shows its name, so it can be moved.
        let editing = self.is_customizing();
        let empty = || {
            if editing {
                customize::placeholder(customize::unit_label(Unit::Item(item)).into(), p)
            } else {
                div().into_any_element()
            }
        };
        match item {
            Item::Menu => self.render_menu_button(place, cx).into_any_element(),
            Item::Sidebar if library::is_enabled(cx) => {
                self.sidebar_toggle(place, cx).into_any_element()
            }
            Item::Folder => self.render_folder_chip(p, cx).unwrap_or_else(empty),
            // In the title bar the title is centered on the window, apart from the other items.
            Item::Title if place == Place::TitleBar => div().into_any_element(),
            Item::Title | Item::FileName | Item::FolderName => {
                self.name_of(item, cx).map_or_else(empty, |name| render_name(name, p))
            }
            _ => empty(),
        }
    }

    /// The name `item` shows now, if there's one: nothing for a file's names while a URL plays.
    fn name_of(&self, item: Item, cx: &App) -> Option<SharedString> {
        if item == Item::Title {
            return Some(self.title(cx).into());
        }
        let player = self.loaded_player(cx)?;
        let file = player.read(cx).media()?.as_path()?.to_path_buf();
        let named = match item {
            Item::FolderName => file.parent()?.to_path_buf(),
            _ => file,
        };
        Some(Media::File(named).display_name().into())
    }
}

/// `name` in line, cut with an ellipsis past [`NAME_WIDTH`].
fn render_name(name: SharedString, p: &Palette) -> AnyElement {
    div()
        .flex_none()
        .max_w(px(NAME_WIDTH))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .text_size(px(NAME_TEXT))
        .text_color(p.text)
        .child(name)
        .into_any_element()
}
