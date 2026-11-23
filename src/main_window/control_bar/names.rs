//! The bar items that aren't icon buttons or playback controls: the names (the title, the
//! file's, its folder's, its artist tag and the saved playlist the queue came from), the
//! Thumbnail (S8.7.6), and the title bar's own items when they are moved (the `…` menu, the
//! sidebar toggle and the folder chip, S8.7.3).
//!
//! The Thumbnail is the playing file's picture from the library, once it has one (a song's
//! cover, a video's frame), or the placeholder for its kind; clicking it shows Properties &
//! Metadata.

use gpui::{AnyElement, App, ClickEvent, Context, SharedString, div, prelude::*, px};

use super::items::Place;
use super::layout::{BUTTON, NAME_WIDTH, THUMBNAIL_WIDTH};
use crate::app::media::Media;
use crate::db::metadata::MediaKind;
use crate::db::thumbnails::SizeClass;
use crate::keymap::actions::ShowProperties;
use crate::library;
use crate::library::folder_view::kind_by_extension;
use crate::library::thumbnails::{self, Status};
use crate::main_window::{MainWindow, customize};
use crate::player_layout::{Item, Unit};
use crate::theme::Palette;
use crate::ui::thumbnail::{Placeholder, ThumbnailTile};
use crate::ui::tooltip;

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
            Item::Thumbnail => self.render_thumbnail(cx),
            Item::Title
            | Item::FileName
            | Item::FolderName
            | Item::ArtistName
            | Item::PlaylistName => {
                self.name_of(item, cx).map_or_else(empty, |name| render_name(name, p))
            }
            _ => empty(),
        }
    }

    /// The name `item` shows now, if there's one: nothing for a file's names while a URL plays.
    fn name_of(&self, item: Item, cx: &App) -> Option<SharedString> {
        match item {
            Item::Title => return Some(self.title(cx).into()),
            Item::PlaylistName => return self.queue.title().map(|title| title.to_owned().into()),
            _ => {}
        }
        let player = self.loaded_player(cx)?;
        if item == Item::ArtistName {
            return player.read(cx).metadata().artist.clone().map(Into::into);
        }
        let file = player.read(cx).media()?.as_path()?.to_path_buf();
        let named = match item {
            Item::FolderName => file.parent()?.to_path_buf(),
            _ => file,
        };
        Some(Media::File(named).display_name().into())
    }
}

impl MainWindow {
    /// The playing file's picture, which shows its properties when clicked.
    fn render_thumbnail(&self, cx: &mut Context<Self>) -> AnyElement {
        let player = self.loaded_player(cx);
        let path = player.as_ref().and_then(|player| player.read(cx).media()?.as_path());
        let song = path.map(kind_by_extension) == Some(MediaKind::Audio);
        let placeholder = if song { Placeholder::Song } else { Placeholder::Video };
        let picture = self.playing_id(cx).and_then(|id| match thumbnails::status(id, cx) {
            Status::Ready(thumbnail) => Some(thumbnail),
            Status::Missing | Status::Pending => None,
        });
        let tile = ThumbnailTile::new(SizeClass::List, placeholder)
            .size(THUMBNAIL_WIDTH, BUTTON)
            .picture(picture);
        div()
            .id("bar-thumbnail")
            .flex_none()
            .cursor_pointer()
            .tooltip(tooltip::text("Properties & Metadata", None::<SharedString>))
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                window.dispatch_action(Box::new(ShowProperties), cx);
            }))
            .child(tile)
            .into_any_element()
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
