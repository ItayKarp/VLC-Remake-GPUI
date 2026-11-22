//! A media card in the listing (S4.6.3; `media_cards/*`, `player_04`): the thumbnail, the file
//! name, and how far it was watched.
//!
//! Watched media is dimmed, and the thumbnail carries an orange bar of the progress along its
//! bottom, full width once watched; media of unknown length has no bar (D9). Hovering brightens
//! the name, shows a play button over the thumbnail and `•••` at the end, which opens the card's
//! file menu ([`file_menu`](super::file_menu)); so does a right-click.

use gpui::{
    App, ClickEvent, Context, Div, Hsla, MouseButton, MouseDownEvent, Stateful, div, prelude::*,
    px, relative,
};

use super::MainWindow;
use super::listing::card_key;
use super::row::row_frame;
use crate::assets::icons;
use crate::db::metadata::MediaKind;
use crate::db::thumbnails::SizeClass;
use crate::library::folder_view::Card;
use crate::library::thumbnails::{self, Status};
use crate::library::watch_state::WatchState;
use crate::theme::Palette;
use crate::ui::icon::icon;
use crate::ui::thumbnail::{self, Placeholder, ThumbnailTile};

/// The group the hover styles of a card's parts follow.
pub(super) const GROUP: &str = "library-card";
/// The watched bar's height (`media_cards/media-watched`).
const BAR_HEIGHT: f32 = 2.;
/// The play button's icon over a list and a grid thumbnail, and how dark the thumbnail goes
/// behind it.
const PLAY_ICON: f32 = 16.;
const GRID_PLAY_ICON: f32 = 28.;
const PLAY_SCRIM: f32 = 0.55;
/// The `•••` button's icon.
const MORE_ICON: f32 = 18.;

impl MainWindow {
    /// The card of `card`, item `index` of the listing, filled when selected.
    pub(super) fn render_card(
        &self,
        index: usize,
        card: &Card,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.library_panel.selection.contains(&card_key(card));
        let frame = row_frame(("library-card", index), p).when(selected, |row| row.bg(p.selected));
        let size = thumbnail::logical_size(SizeClass::List);
        let frame = card_events(frame, index, cx);
        super::drag::draggable(frame, index, cx)
            .child(thumbnail_stack(card, SizeClass::List, size, p, cx))
            .child(card_name(card, p))
            .child(more_button(index, p.icon, cx))
    }
}

/// `frame` as item `index`'s card: its parts hover together, a click selects or opens it, and
/// a right-click opens its menu.
pub(super) fn card_events(
    frame: Stateful<Div>,
    index: usize,
    cx: &mut Context<MainWindow>,
) -> Stateful<Div> {
    frame
        .group(GROUP)
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.click_item(index, event, window, cx);
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                this.open_item_menu(index, event.position, window, cx);
            }),
        )
}

/// A card's name: dim when watched, brighter while the card is hovered.
pub(super) fn card_name(card: &Card, p: &Palette) -> Div {
    let strong = p.text_strong;
    div()
        .flex_1()
        .min_w_0()
        .truncate()
        .when(card.watch.is_watched(), |name| name.text_color(p.text_muted))
        .group_hover(GROUP, move |style| style.text_color(strong))
        .child(card.label.clone())
}

/// The thumbnail of `class`'s picture drawn `width` by `height`, with the watched bar and the
/// hover play button over it.
pub(super) fn thumbnail_stack(
    card: &Card,
    class: SizeClass,
    (width, height): (f32, f32),
    p: &Palette,
    cx: &App,
) -> Div {
    let placeholder =
        if card.kind == MediaKind::Audio { Placeholder::Song } else { Placeholder::Video };
    let picture = card.media.and_then(|media| match thumbnails::status(media, cx) {
        Status::Ready(thumbnail) => Some(thumbnail),
        Status::Missing | Status::Pending => None,
    });
    let play_icon = match class {
        SizeClass::List => PLAY_ICON,
        SizeClass::Grid => GRID_PLAY_ICON,
    };
    let play = div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::black().opacity(PLAY_SCRIM))
        .opacity(0.)
        .group_hover(GROUP, |style| style.opacity(1.))
        .child(icon(icons::PLAY, play_icon, gpui::white()));
    div()
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .child(ThumbnailTile::new(class, placeholder).size(width, height).picture(picture))
        .children(progress(card.watch).map(|fraction| {
            div()
                .absolute()
                .left_0()
                .bottom_0()
                .h(px(BAR_HEIGHT))
                .w(relative(fraction))
                .bg(p.accent)
        }))
        .child(play)
}

/// How much of the thumbnail's width the watched bar covers, if it has one (D9).
fn progress(watch: WatchState) -> Option<f32> {
    match watch {
        WatchState::Unwatched => None,
        WatchState::InProgress(fraction) => Some(fraction),
        WatchState::Watched => Some(1.),
    }
}

/// `•••` in `color`, shown while item `index` is hovered, which opens its menu below the
/// pointer.
pub(super) fn more_button(
    index: usize,
    color: Hsla,
    cx: &mut Context<MainWindow>,
) -> Stateful<Div> {
    div()
        .id(("library-card-more", index))
        .flex_none()
        .px(px(6.))
        .cursor_pointer()
        .opacity(0.)
        .group_hover(GROUP, |style| style.opacity(1.))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            cx.stop_propagation();
            this.open_item_menu(index, event.position(), window, cx);
        }))
        .child(icon(icons::MORE, MORE_ICON, color))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_shows_real_progress_and_full_width_when_watched() {
        assert_eq!(progress(WatchState::Unwatched), None);
        assert_eq!(progress(WatchState::InProgress(0.25)), Some(0.25));
        assert_eq!(progress(WatchState::Watched), Some(1.));
    }
}
