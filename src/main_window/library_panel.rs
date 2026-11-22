//! The library sidebar over the player (S4.5, S4.6; `player_02`, `library/sidebar/*`): the icon
//! rail ([`rail`]), and a panel with back and forward, the breadcrumb ([`crumbs`]), the item count
//! ([`header`]) and what the current route lists ([`listing`]): rows ([`row`]) and media cards
//! ([`card`], [`card_menu`]) in the order [`sort_menu`] picks, or the same as tiles ([`grid`],
//! chosen with [`view_menu`]), or what a search finds ([`search_field`]), or why there is
//! nothing ([`empty`]),
//! browsed with the mouse and the keyboard ([`browse`]), and dragged out ([`drag`]).
//!
//! The title bar's sidebar toggle and L show and hide it. The navigation model is
//! [`crate::navigation`]; the panel keeps a [`NavHistory`] and remembers the overlay's last
//! route across launches. A Videos or Songs folder lists its subfolders and its media; Music's
//! categories, playlists and History list the places they lead to and what they play. Mouse buttons 4
//! and 5, Alt+Left and Alt+Right (and Cmd+[ and ] on macOS) go back and forward while the
//! panel has the keyboard.

mod add_menu;
mod browse;
mod card;
mod card_menu;
mod crumbs;
mod drag;
mod empty;
mod grid;
mod header;
mod listing;
mod rail;
mod row;
mod search_field;
mod selection;
mod sort_menu;
mod view_menu;

pub(super) use self::drag::DraggedMedia;

use gpui::{
    App, Context, Div, Entity, FocusHandle, MouseButton, MouseDownEvent, NavigationDirection,
    ScrollStrategy, Stateful, Subscription, UniformListScrollHandle, Window, div, prelude::*, px,
};

use self::card_menu::CardMenu;
use self::listing::FolderListing;
use self::search_field::SearchField;
use self::selection::Selection;
use super::MainWindow;
use crate::db::media::MediaId;
use crate::keymap::actions::{GoBack, GoForward};
use crate::keymap::contexts;
use crate::library::thumbnails::Thumbnails;
use crate::navigation::Presentation;
use crate::navigation::history::{NavHistory, Step};
use crate::navigation::route::Route;
use crate::playlists;
use crate::settings;
use crate::theme::Palette;
use crate::ui::focus;
use crate::ui::menu::ContextMenu;
use crate::ui::popover::BoundsTracker;

/// The rail's width, and the panel's beside it (`player_02`).
const RAIL_WIDTH: f32 = 50.;
const PANEL_WIDTH: f32 = 377.;
/// The panel's side padding.
const INSET: f32 = 20.;

/// A menu the panel opened, and the subscription that forgets it once dismissed.
type OpenMenu = Option<(Entity<ContextMenu>, Subscription)>;

/// The sidebar's state: whether it shows, where it is, and its menus.
pub(super) struct LibraryPanel {
    open: bool,
    history: NavHistory,
    focus: FocusHandle,
    /// What the folder the route shows holds, read in the background.
    folder: Option<FolderListing>,
    /// The listing's keyboard focus, its scroll position and its selected item.
    list_focus: FocusHandle,
    scroll: UniformListScrollHandle,
    selection: Selection,
    /// The media whose cards were on screen, as the thumbnailer was last told.
    on_screen: Vec<MediaId>,
    /// A card's menu.
    card_menu: CardMenu,
    /// The breadcrumb's `…` menu of hidden segments, and its trigger.
    crumb_menu: OpenMenu,
    crumb_button: BoundsTracker,
    /// The rail's "+" menu, and its button.
    add_menu: OpenMenu,
    add_button: BoundsTracker,
    /// The header's Sort menu, and its button.
    sort_menu: OpenMenu,
    sort_button: BoundsTracker,
    /// The header's View menu, and its button.
    view_menu: OpenMenu,
    view_button: BoundsTracker,
    /// The search field, while open.
    search: Option<SearchField>,
}

impl LibraryPanel {
    /// A closed sidebar at the overlay's last route.
    pub(super) fn new(cx: &mut Context<MainWindow>) -> Self {
        // A folder may appear or vanish when the indexer reports a change.
        cx.observe_global::<crate::library::Library>(MainWindow::reload_folder).detach();
        // Thumbnails show as they arrive.
        cx.observe_global::<Thumbnails>(|this, cx| {
            if this.library_panel.open {
                cx.notify();
            }
        })
        .detach();
        let routes = &settings::get(cx).local.library_routes;
        Self {
            open: false,
            history: NavHistory::new(Presentation::Overlay.last_route(routes)),
            focus: cx.focus_handle(),
            folder: None,
            list_focus: cx.focus_handle().tab_stop(true),
            scroll: UniformListScrollHandle::new(),
            selection: Selection::default(),
            on_screen: Vec::new(),
            card_menu: None,
            crumb_menu: None,
            crumb_button: BoundsTracker::new(),
            add_menu: None,
            add_button: BoundsTracker::new(),
            sort_menu: None,
            sort_button: BoundsTracker::new(),
            view_menu: None,
            view_button: BoundsTracker::new(),
            search: None,
        }
    }

    /// Whether the sidebar shows.
    pub(super) fn is_open(&self) -> bool {
        self.open
    }
}

/// Whether `route` still exists: a deleted playlist doesn't. Folders always do; a missing one
/// says so when shown.
fn exists(route: &Route, cx: &App) -> bool {
    match route {
        Route::Playlists(Some(id)) => playlists::summaries(cx).iter().any(|p| p.id == *id),
        _ => true,
    }
}

impl MainWindow {
    /// Shows or hides the sidebar (the title bar toggle and L).
    pub(super) fn toggle_library_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let panel = &mut self.library_panel;
        panel.open = !panel.open;
        panel.crumb_menu = None;
        panel.add_menu = None;
        panel.card_menu = None;
        panel.sort_menu = None;
        panel.view_menu = None;
        panel.search = None;
        if panel.open {
            self.reload_folder(cx);
            let handle = self.library_panel.list_focus.clone();
            window.defer(cx, move |window, cx| handle.focus(window, cx));
        } else if self.library_panel.focus.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// Goes to `route`, remembering it for the next launch. Going somewhere closes the search
    /// field.
    pub(super) fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        self.library_panel.search = None;
        if self.library_panel.history.go(route) {
            self.arrived(cx);
        }
    }

    /// Takes one step back or forward, past routes that no longer exist.
    pub(super) fn step_library(&mut self, step: Step, cx: &mut Context<Self>) {
        let exists = |route: &Route| exists(route, cx);
        if self.library_panel.history.step(step, exists) {
            self.arrived(cx);
        }
    }

    /// Whether one `step` leads somewhere (the Back and Forward buttons' states).
    fn can_step_library(&self, step: Step, cx: &App) -> bool {
        self.library_panel.history.can_step(step, |route| exists(route, cx))
    }

    /// Lists the new route and saves it as the overlay's last.
    fn arrived(&mut self, cx: &mut Context<Self>) {
        let panel = &mut self.library_panel;
        panel.crumb_menu = None;
        panel.card_menu = None;
        panel.sort_menu = None;
        panel.view_menu = None;
        panel.selection.clear();
        panel.scroll.scroll_to_item(0, ScrollStrategy::Top);
        self.reload_folder(cx);
        let route = self.library_panel.history.current().clone();
        settings::update(cx, |s| {
            Presentation::Overlay.remember(&route, &mut s.local.library_routes);
        });
        cx.notify();
    }

    /// The sidebar over the left of the content area, `top` below the window's top and `bottom`
    /// above its bottom, while it shows.
    pub(super) fn render_library_panel(
        &self,
        p: &Palette,
        (top, bottom): (f32, f32),
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        if !self.library_panel.open {
            return None;
        }
        let panel = div()
            .w(px(PANEL_WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .bg(p.panel)
            .child(self.render_library_header(p, cx))
            .child(self.render_listing(p, cx));
        let root = div()
            .id("library-panel")
            .key_context(focus::context(&[contexts::LIBRARY]))
            .track_focus(&self.library_panel.focus)
            .absolute()
            .left_0()
            .top(px(top))
            .bottom(px(bottom))
            .max_w_full()
            .w(px(RAIL_WIDTH + PANEL_WIDTH))
            .flex()
            .flex_row()
            .overflow_hidden()
            .text_color(p.text)
            .occlude()
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.step_library(Step::Back, cx)))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| {
                this.step_library(Step::Forward, cx);
            }))
            .child(self.render_rail(p, cx))
            .child(panel);
        Some(mouse_navigation(root, cx))
    }

    /// The panel's menus that are open: the breadcrumb's `…`, the rail's "+", Sort, View and a
    /// card's.
    pub(super) fn render_library_menus(&self) -> impl Iterator<Item = gpui::AnyElement> {
        let panel = &self.library_panel;
        let crumbs = crumbs::render_menu(&panel.crumb_menu, &panel.crumb_button);
        let add = add_menu::render(&panel.add_menu, &panel.add_button);
        let sort = sort_menu::render(&panel.sort_menu, &panel.sort_button);
        let view = view_menu::render(&panel.view_menu, &panel.view_button);
        let menus = crumbs.into_iter().chain(add).chain(sort).chain(view);
        menus.chain(card_menu::render(&panel.card_menu))
    }
}

/// Mouse buttons 4 and 5 go back and forward (S4.5.2).
fn mouse_navigation(root: Stateful<Div>, cx: &mut Context<MainWindow>) -> Stateful<Div> {
    let button = |direction| MouseButton::Navigate(direction);
    root.on_mouse_down(
        button(NavigationDirection::Back),
        cx.listener(|this, _: &MouseDownEvent, _, cx| this.step_library(Step::Back, cx)),
    )
    .on_mouse_down(
        button(NavigationDirection::Forward),
        cx.listener(|this, _: &MouseDownEvent, _, cx| this.step_library(Step::Forward, cx)),
    )
}

#[cfg(test)]
mod tests;
