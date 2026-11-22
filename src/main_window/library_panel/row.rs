//! The listing's plain rows (`player_02`, `sidebar-videos*`): an icon and a name, for a place to
//! go to, a recently played media, a network stream (S4.11), or the "+ Add" and "+ Add Network
//! Stream" actions (S4.6.1). A folder's, a saved playlist's and a stream's row has a menu
//! (S4.8.2, S4.8.3), from a right-click or its `•••` on hover.

use gpui::{
    ClickEvent, Context, MouseButton, MouseDownEvent, SharedString, Stateful, Window, div,
    prelude::*, px,
};

use super::MainWindow;
use super::add_menu::{self, Shelf};
use super::card::{GROUP, more_button};
use super::item_menu::MenuKind;
use super::selection::Key;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::assets::icons;
use crate::navigation::route::{MusicRoute, NetworkCategory, Route};
use crate::network::{self, links};
use crate::theme::Palette;
use crate::ui::{icon, metrics};

/// A row's height, its icon box and the gap before its text (`player_02`).
pub(super) const ROW_HEIGHT: f32 = 42.;
pub(super) const ROW_ICON_BOX: f32 = 32.;
pub(super) const ROW_GAP: f32 = 10.;
pub(super) const ROW_TEXT: f32 = 14.;

/// One row: what it shows and what clicking it does.
#[derive(Clone)]
pub(super) struct Row {
    label: SharedString,
    icon: &'static str,
    target: Target,
}

/// What clicking a row does.
#[derive(Clone)]
pub(super) enum Target {
    /// Goes to a place.
    Go(Route),
    /// Plays a media.
    Play(Media),
    /// Asks for a folder and adds it to the library's video or music folders.
    Add(Shelf),
    /// Plays a stream listed in a Network category.
    Stream(String, NetworkCategory),
    /// Opens the Open Network Stream dialog.
    AddStream,
}

impl Row {
    /// A row named `label` with `icon`, doing `target`.
    pub(super) fn new(label: impl Into<SharedString>, icon: &'static str, target: Target) -> Self {
        Self { label: label.into(), icon, target }
    }

    /// A recently played media, which plays when clicked.
    pub(super) fn recent(media: Media) -> Self {
        let icon = if media.as_path().is_some() { icons::FILE } else { icons::STREAMING };
        Self::new(media.display_name(), icon, Target::Play(media))
    }

    /// A stream listed in Network's `category`, which plays when clicked. A password in it is
    /// masked.
    pub(super) fn stream(url: String, category: NetworkCategory) -> Self {
        Self::new(links::masked(&url), icons::STREAMING, Target::Stream(url, category))
    }

    /// "+ Add" for a Videos or Songs route; other routes have none.
    pub(super) fn add(route: &Route) -> Option<Self> {
        let shelf = match route {
            Route::Videos(_) => Shelf::Videos,
            Route::Music(MusicRoute::Songs(_)) => Shelf::Music,
            _ => return None,
        };
        Some(Self::new("Add", icons::ADD, Target::Add(shelf)))
    }

    /// The row's name.
    pub(super) fn label(&self) -> &SharedString {
        &self.label
    }

    /// The row's icon.
    pub(super) fn icon(&self) -> &'static str {
        self.icon
    }

    /// What clicking the row does.
    pub(super) fn target(&self) -> &Target {
        &self.target
    }

    /// Whether the row is an action rather than an item.
    pub(super) fn is_action(&self) -> bool {
        matches!(self.target, Target::Add(_) | Target::AddStream)
    }

    /// The menu the row has: a folder's or a saved playlist's.
    pub(super) fn menu(&self) -> Option<MenuKind> {
        match &self.target {
            Target::Go(Route::Playlists(Some(_))) => Some(MenuKind::Playlist),
            Target::Go(route) if route.folder().is_some() => Some(MenuKind::Folder),
            Target::Stream(..) => Some(MenuKind::Stream),
            _ => None,
        }
    }

    /// What the row is, for the selection.
    pub(super) fn key(&self) -> Key {
        match &self.target {
            Target::Go(route) => Key::Place(route.clone()),
            Target::Play(media) => Key::Recent(media.clone()),
            Target::Stream(url, _) => Key::Recent(Media::Url(url.clone())),
            Target::Add(_) | Target::AddStream => Key::Add,
        }
    }

    /// The row at `index`, filled when `selected`.
    pub(super) fn render(
        &self,
        index: usize,
        selected: bool,
        p: &Palette,
        cx: &mut Context<MainWindow>,
    ) -> Stateful<gpui::Div> {
        let (hover, strong) = (p.hover, p.text_strong);
        let target = self.target.clone();
        let frame = row_frame(("library-row", index), p)
            .debug_selector(move || format!("library-row-{index}"));
        let frame = super::drag::drop_target(frame, self, p, cx);
        let frame = menu_events(frame, self, index, cx);
        frame
            .when(selected, |row| row.bg(p.selected))
            .hover(move |style| style.bg(hover).text_color(strong))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.follow(target.clone(), window, cx);
            }))
            .child(
                div()
                    .size(px(ROW_ICON_BOX))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .child(icon::icon(self.icon, metrics::ICON, p.icon)),
            )
            .child(div().flex_1().min_w_0().truncate().child(self.label.clone()))
            .when(self.menu().is_some(), |row| row.child(more_button(index, p.icon, cx)))
    }
}

/// `frame` as item `index`'s `row`: when it has a menu, its parts hover together and a
/// right-click opens the menu.
pub(super) fn menu_events(
    frame: Stateful<gpui::Div>,
    row: &Row,
    index: usize,
    cx: &mut Context<MainWindow>,
) -> Stateful<gpui::Div> {
    if row.menu().is_none() {
        return frame;
    }
    frame.group(GROUP).on_mouse_down(
        MouseButton::Right,
        cx.listener(move |this, event: &MouseDownEvent, window, cx| {
            this.open_item_menu(index, event.position, window, cx);
        }),
    )
}

/// A listing row's frame: its height, its insets and its spacing.
pub(super) fn row_frame(id: impl Into<gpui::ElementId>, p: &Palette) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(ROW_HEIGHT))
        .w_full()
        .px(px(2.))
        .flex()
        .items_center()
        .gap(px(ROW_GAP))
        .rounded(px(metrics::RADIUS))
        .text_size(px(ROW_TEXT))
        .text_color(p.text)
}

impl MainWindow {
    /// Does what a row says: goes to its route, plays its media or stream, adds a folder, or
    /// opens Open Network Stream. A recent stream played moves to the top of its list.
    pub(super) fn follow(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Go(route) => self.navigate(route, cx),
            Target::Play(media) => {
                let placement = self.library_placement(cx);
                self.open(OpenRequest::placed(vec![media], placement), window, cx);
            }
            Target::Add(folders) => add_menu::add_folder(cx.entity().downgrade(), folders, cx),
            Target::Stream(url, category) => {
                if category == NetworkCategory::Recent {
                    network::record_recent(std::slice::from_ref(&url), cx);
                }
                let placement = self.library_placement(cx);
                self.open(OpenRequest::placed(vec![Media::Url(url)], placement), window, cx);
            }
            Target::AddStream => self.show_network_stream(window, cx),
        }
    }
}
