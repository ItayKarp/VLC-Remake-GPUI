//! The library over the player (S4.5, S4.6; `player_02`, `library/sidebar/*`) and as full library
//! mode (S4.10; `player_03`–`07`). The sidebar is the icon rail ([`rail`]) and a panel with back
//! and forward, the breadcrumb ([`crumbs`]), the item count ([`header`]) and what the current
//! route lists ([`listing`]): Home's sections ([`home`]), rows ([`row`]) and media cards
//! ([`card`]) in the order [`sort_menu`] picks, or the same as tiles ([`grid`], chosen with
//! [`view_menu`]), or what a search finds ([`search_field`]), or why there is nothing
//! ([`empty`]), browsed with the mouse and the keyboard ([`browse`]), and dragged out
//! ([`drag`]). Cards, folders and playlists have menus ([`item_menu`]: [`file_menu`],
//! [`folder_menu`], [`playlist_menu`]). Full library mode ([`full`]) shows the same listing
//! across the window, laid out for its width ([`layout`]).
//!
//! The title bar's sidebar toggle and L show and hide the sidebar, and its pop-out switches to
//! full library mode ([`mode`]). The navigation model is [`crate::navigation`]; the library keeps
//! a [`NavHistory`] and remembers each presentation's last route across launches. A Videos or
//! Songs folder lists its subfolders and its media; Music's categories, playlists and History
//! list the places they lead to and what they play. Mouse buttons 4 and 5, Alt+Left and
//! Alt+Right (and Cmd+[ and ] on macOS) go back and forward while the library has the keyboard.

mod add_menu;
mod browse;
mod card;
mod crumbs;
mod drag;
mod empty;
mod file_menu;
mod folder_menu;
mod full;
mod grid;
mod header;
mod home;
mod item_menu;
mod layout;
mod listing;
mod mode;
mod playlist_menu;
mod podcast_menu;
mod rail;
mod row;
mod search_field;
mod selection;
mod sort_menu;
mod stream_menu;
mod view_menu;

pub(super) use self::drag::DraggedMedia;
pub(super) use self::file_menu::show_in_folder_label;

use gpui::{
    App, Context, Div, Entity, FocusHandle, MouseButton, MouseDownEvent, NavigationDirection,
    ScrollHandle, ScrollStrategy, Stateful, Subscription, UniformListScrollHandle, Window, div,
    prelude::*, px,
};

use self::item_menu::ItemMenu;
use self::layout::Shape;
use self::listing::FolderListing;
use self::search_field::SearchField;
use self::selection::Selection;
use super::MainWindow;
use crate::db::media::MediaId;
use crate::i18n::DirectionalStyle;
use crate::keymap::actions::{GoBack, GoForward};
use crate::keymap::contexts;
use crate::library::thumbnails::Thumbnails;
use crate::navigation::Presentation;
use crate::navigation::history::{NavHistory, Step};
use crate::navigation::route::Route;
use crate::navigation::view::View;
use crate::playlists;
use crate::settings;
use crate::theme::Palette;
use crate::ui::focus;
use crate::ui::menu::ContextMenu;
use crate::ui::popover::BoundsTracker;

/// The rail's width, and the panel's beside it (`player_02`).
pub(super) const RAIL_WIDTH: f32 = 50.;
pub(super) const PANEL_WIDTH: f32 = 377.;
/// The panel's side padding.
const INSET: f32 = 20.;

/// A menu the panel opened, and the subscription that forgets it once dismissed.
type OpenMenu = Option<(Entity<ContextMenu>, Subscription)>;

/// The library's state: how it shows, where it is, and its menus.
pub(super) struct LibraryPanel {
    /// How the library shows, while it does: over the player or as full library mode.
    shown: Option<Presentation>,
    /// The window's width when last drawn, which the listing's layout follows.
    window_width: f32,
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
    /// A card's, folder's or playlist's menu.
    item_menu: ItemMenu,
    /// The item Go to Source selects once its folder is read.
    reveal: Option<selection::Key>,
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
    /// Home as read, and its page's scroll position.
    home: Option<home::HomeListing>,
    home_scroll: ScrollHandle,
}

impl LibraryPanel {
    /// A closed sidebar at the overlay's last route.
    pub(super) fn new(cx: &mut Context<MainWindow>) -> Self {
        // A folder may appear or vanish when the indexer reports a change.
        cx.observe_global::<crate::library::Library>(MainWindow::reload_folder).detach();
        // Network lists its streams as they are saved.
        cx.observe_global::<crate::network::Streams>(|_, cx| cx.notify()).detach();
        // Thumbnails show as they arrive.
        cx.observe_global::<Thumbnails>(|this, cx| {
            if this.library_panel.is_open() {
                cx.notify();
            }
        })
        .detach();
        let routes = &settings::get(cx).local.library_routes;
        Self {
            shown: None,
            window_width: 0.,
            history: NavHistory::new(Presentation::Overlay.last_route(routes)),
            focus: cx.focus_handle(),
            folder: None,
            list_focus: cx.focus_handle().tab_stop(true),
            scroll: UniformListScrollHandle::new(),
            selection: Selection::default(),
            on_screen: Vec::new(),
            item_menu: None,
            reveal: None,
            crumb_menu: None,
            crumb_button: BoundsTracker::new(),
            add_menu: None,
            add_button: BoundsTracker::new(),
            sort_menu: None,
            sort_button: BoundsTracker::new(),
            view_menu: None,
            view_button: BoundsTracker::new(),
            search: None,
            home: None,
            home_scroll: ScrollHandle::new(),
        }
    }

    /// Whether the library shows, over the player or as full library mode.
    pub(super) fn is_open(&self) -> bool {
        self.shown.is_some()
    }

    /// Whether full library mode shows (S4.10.2).
    pub(super) fn is_full(&self) -> bool {
        self.shown == Some(Presentation::Full)
    }

    /// How the library shows, or would show when opened: over the player.
    fn presentation(&self) -> Presentation {
        self.shown.unwrap_or(Presentation::Overlay)
    }

    /// Whether one of the library's menus is open.
    pub(super) fn has_open_menu(&self) -> bool {
        self.item_menu.is_some()
            || self.crumb_menu.is_some()
            || self.add_menu.is_some()
            || self.sort_menu.is_some()
            || self.view_menu.is_some()
    }

    /// Closes the library's menus.
    fn close_menus(&mut self) {
        self.crumb_menu = None;
        self.add_menu = None;
        self.item_menu = None;
        self.sort_menu = None;
        self.view_menu = None;
    }

    /// How the listing lays out `view` at the window's width.
    fn shape(&self, view: View) -> Shape {
        let presentation = self.presentation();
        Shape::fit(layout::list_width(self.window_width, presentation), view, presentation)
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

    /// Lists the new route and saves it as the presentation's last.
    fn arrived(&mut self, cx: &mut Context<Self>) {
        let panel = &mut self.library_panel;
        panel.close_menus();
        panel.selection.clear();
        panel.scroll.scroll_to_item(0, ScrollStrategy::Top);
        panel.home_scroll.set_offset(gpui::Point::default());
        self.reload_folder(cx);
        let route = self.library_panel.history.current().clone();
        crate::library::home::writes::record_visit(&route, cx);
        let presentation = self.library_panel.presentation();
        settings::update(cx, |s| presentation.remember(&route, &mut s.local.library_routes));
        cx.notify();
    }

    /// The library while it shows, `top` below the window's top and `bottom` above its bottom:
    /// the sidebar over the left of the player, or full library mode across the window.
    pub(super) fn render_library_panel(
        &self,
        p: &Palette,
        (top, bottom): (f32, f32),
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let presentation = self.library_panel.shown?;
        let root = div()
            .id("library-panel")
            .key_context(focus::context(&[contexts::LIBRARY]))
            .track_focus(&self.library_panel.focus)
            .absolute()
            .inset_start(px(0.))
            .top(px(top))
            .bottom(px(bottom))
            .flex_row_start()
            .overflow_hidden()
            .text_color(p.text)
            .occlude()
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.step_library(Step::Back, cx)))
            .on_action(cx.listener(|this, _: &GoForward, _, cx| {
                this.step_library(Step::Forward, cx);
            }));
        let root = match presentation {
            Presentation::Overlay => self.render_overlay(root, p, cx),
            Presentation::Full => self.render_full_library(root, p, cx),
        };
        Some(mouse_navigation(root, cx))
    }

    /// The rail and the panel beside it, the whole window wide when it is narrow.
    fn render_overlay(
        &self,
        root: Stateful<Div>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let panel = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex_row_start()
            .flex_col()
            .bg(p.panel)
            .child(self.render_library_header(p, cx))
            .child(self.render_listing(p, cx));
        let width = layout::overlay_width(self.library_panel.window_width);
        root.w(px(width)).child(self.render_rail(p, cx)).child(panel)
    }

    /// Gives the listing the keyboard.
    pub(super) fn focus_library_listing(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.library_panel.list_focus.focus(window, cx);
    }

    /// How much of the window's left full library mode's navigation takes.
    pub(super) fn library_nav_space() -> f32 {
        layout::nav_space()
    }

    /// Lays the library out for `window`'s width from now on.
    pub(super) fn fit_library(&mut self, window: &Window, cx: &App) {
        self.library_panel.window_width = f32::from(window.viewport_size().width);
        self.refit_home(cx);
    }

    /// Whether the pointer is over the sidebar over the player, which holds the controls up.
    pub(super) fn pointer_over_library(&self, window: &Window) -> bool {
        let panel = &self.library_panel;
        let Some(pointer) = self.auto_hide.pointer else { return false };
        panel.shown == Some(Presentation::Overlay)
            && f32::from(pointer.x) < layout::overlay_width(f32::from(window.viewport_size().width))
    }

    /// The panel's menus that are open: the breadcrumb's `…`, the rail's "+", Sort, View and an
    /// item's.
    pub(super) fn render_library_menus(&self) -> impl Iterator<Item = gpui::AnyElement> {
        let panel = &self.library_panel;
        let crumbs = crumbs::render_menu(&panel.crumb_menu, &panel.crumb_button);
        let add = add_menu::render(&panel.add_menu, &panel.add_button);
        let sort = sort_menu::render(&panel.sort_menu, &panel.sort_button);
        let view = view_menu::render(&panel.view_menu, &panel.view_button);
        let menus = crumbs.into_iter().chain(add).chain(sort).chain(view);
        menus.chain(item_menu::render(&panel.item_menu))
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
