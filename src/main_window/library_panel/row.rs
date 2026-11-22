//! The listing's plain rows (`player_02`, `sidebar-videos*`): an icon and a name, for a place to
//! go to, a recently played media, or the "+ Add" action (S4.6.1).

use gpui::{ClickEvent, Context, SharedString, Stateful, Window, div, prelude::*, px};

use super::MainWindow;
use super::add_menu::{self, Shelf};
use super::selection::Key;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::assets::icons;
use crate::navigation::route::{MusicRoute, Route};
use crate::theme::Palette;
use crate::ui::{icon, metrics};

/// A row's height, its icon box and the gap before its text (`player_02`).
pub(super) const ROW_HEIGHT: f32 = 42.;
pub(super) const ROW_ICON_BOX: f32 = 32.;
pub(super) const ROW_GAP: f32 = 10.;
pub(super) const ROW_TEXT: f32 = 14.;

/// One row: what it shows and what clicking it does.
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
        matches!(self.target, Target::Add(_))
    }

    /// What the row is, for the selection.
    pub(super) fn key(&self) -> Key {
        match &self.target {
            Target::Go(route) => Key::Place(route.clone()),
            Target::Play(media) => Key::Recent(media.clone()),
            Target::Add(_) => Key::Add,
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
        super::drag::drop_target(row_frame(("library-row", index), p), self, p, cx)
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
            .child(div().min_w_0().truncate().child(self.label.clone()))
    }
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
    /// Does what a row says: goes to its route, plays its media, or adds a folder.
    pub(super) fn follow(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Go(route) => self.navigate(route, cx),
            Target::Play(media) => self.open(OpenRequest::replace(vec![media]), window, cx),
            Target::Add(folders) => add_menu::add_folder(cx.entity().downgrade(), folders, cx),
        }
    }
}
