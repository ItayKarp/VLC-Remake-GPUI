//! The listing's item menus (S4.8): a media card's file menu ([`file_menu`](super::file_menu)),
//! a folder row's ([`folder_menu`](super::folder_menu)) and a playlist row's
//! ([`playlist_menu`](super::playlist_menu)). Each opens from a right-click, the item's `•••`
//! hover button, and the context-menu key or Shift+F10 on the keyboard's item (S4.8.4).
//!
//! A menu acts on every selected item of its kind, in list order, when the item it opened on is
//! selected; on any other item it selects that item alone first. Actions that make no sense for
//! several items are disabled and say why on hover.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, Pixels, Point, Subscription, WeakEntity,
    Window, point, px,
};

use super::grid::LINE_HEIGHT;
use super::listing::{Body, Item};
use super::row::{ROW_HEIGHT, Target};
use super::view_menu::line_of;
use super::{INSET, MainWindow, file_menu, folder_menu, playlist_menu};
use crate::db::media::MediaId;
use crate::db::metadata::MediaKind;
use crate::db::playlists::Summary;
use crate::navigation::route::Route;
use crate::navigation::view::View;
use crate::playlists;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover;

/// The open item menu, where it opened, and the subscription that forgets it once dismissed.
pub(super) type ItemMenu = Option<(Entity<ContextMenu>, Point<Pixels>, Subscription)>;

/// What hovering an action that works on one item at a time says while several are selected.
const ONE_AT_A_TIME: &str = "Works on one item at a time";
/// How far in from the item's left edge a menu opened from the keyboard sits.
const KEYBOARD_INSET: f32 = 24.;

/// The menu an item has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MenuKind {
    /// A media card's.
    File,
    /// A folder row's.
    Folder,
    /// A playlist row's.
    Playlist,
}

/// One media card a file menu acts on.
#[derive(Clone)]
pub(super) struct FileTarget {
    /// The file.
    pub(super) path: PathBuf,
    /// Its media, once the library knows it.
    pub(super) media: Option<MediaId>,
    /// Whether it is audio or video.
    pub(super) kind: MediaKind,
    /// Whether it was watched to the end.
    pub(super) watched: bool,
    /// Its place in the listing: Play plays its folder from there.
    pub(super) index: usize,
}

/// What a menu acts on, in list order.
enum Subject {
    Files(Vec<FileTarget>),
    Folders(Vec<PathBuf>),
    Playlists(Vec<Summary>),
}

/// The menu `item` has, if any: cards, folders and saved playlists have one.
fn menu_kind(item: &Item<'_>) -> Option<MenuKind> {
    match item {
        Item::Card(_) => Some(MenuKind::File),
        Item::Row(row) => row.menu(),
    }
}

impl MainWindow {
    /// Opens the menu of item `index` (and the items of its kind selected with it) at
    /// `position`.
    pub(super) fn open_item_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(subject) = self.menu_subject(index, cx) else { return };
        let main = cx.entity().downgrade();
        let items = match &subject {
            Subject::Files(files) => {
                let mut items = file_menu::items(&main, files, cx);
                items.splice(1..1, self.continue_watching_item(&main, files));
                items
            }
            Subject::Folders(folders) => folder_menu::items(&main, folders),
            Subject::Playlists(playlists) => playlist_menu::items(&main, playlists, cx),
        };
        let menu = ContextMenu::build(items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.library_panel.item_menu = None;
            cx.notify();
        });
        self.library_panel.item_menu = Some((menu, position, subscription));
        cx.notify();
    }

    /// Opens the keyboard's item's menu under it, with its first action highlighted (the
    /// context-menu key and Shift+F10).
    pub(super) fn open_item_menu_from_keyboard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Select All leaves no keyboard item: the first selected one stands in.
        let Some(index) = self.cursor_index(cx).or_else(|| self.first_selected(cx)) else {
            return;
        };
        let position = self.below_item(index, cx);
        self.open_item_menu(index, position, window, cx);
        if let Some((menu, _, _)) = &self.library_panel.item_menu {
            menu.update(cx, ContextMenu::highlight_first);
        }
    }

    /// The first selected item's index, if any is selected.
    fn first_selected(&self, cx: &App) -> Option<usize> {
        let Body::Items(items) = self.listing(cx) else { return None };
        self.library_panel.selection.indices(&items.keys()).first().copied()
    }

    /// Where a menu opened from the keyboard goes: under item `index`, inside the listing.
    fn below_item(&self, index: usize, cx: &App) -> Point<Pixels> {
        let home = self.home_child(index);
        if let Some(item) =
            home.and_then(|child| self.library_panel.home_scroll.bounds_for_item(child))
        {
            return point(item.left() + px(KEYBOARD_INSET), item.bottom());
        }
        let view = self.library_view(cx);
        let line_height = match view {
            View::List => ROW_HEIGHT,
            View::Grid => LINE_HEIGHT,
        };
        let scroll = self.library_panel.scroll.0.borrow();
        let (bounds, offset) = (scroll.base_handle.bounds(), scroll.base_handle.offset());
        let below = px((line_of(index, view) + 1) as f32 * line_height);
        let y = (bounds.top() + offset.y + below).clamp(bounds.top(), bounds.bottom());
        point(bounds.left() + px(INSET + KEYBOARD_INSET), y)
    }

    /// What item `index`'s menu acts on, selecting it alone when it isn't selected.
    fn menu_subject(&mut self, index: usize, cx: &mut Context<Self>) -> Option<Subject> {
        let Body::Items(items) = self.listing(cx) else { return None };
        let kind = menu_kind(&items.get(index)?)?;
        let key = items.keys().get(index)?.clone();
        if !self.library_panel.selection.contains(&key) {
            self.library_panel.selection.select(key);
        }
        let Body::Items(items) = self.listing(cx) else { return None };
        let picked = self.library_panel.selection.indices(&items.keys()).into_iter();
        let picked = picked.filter_map(|at| Some((at, items.get(at)?)));
        let picked: Vec<_> = picked.filter(|(_, item)| menu_kind(item) == Some(kind)).collect();
        Some(subject(kind, &picked, cx))
    }
}

/// The `picked` items, all of `kind`, as what their menu acts on.
fn subject(kind: MenuKind, picked: &[(usize, Item<'_>)], cx: &App) -> Subject {
    let routes = || {
        picked.iter().filter_map(|(_, item)| match item {
            Item::Row(row) => match row.target() {
                Target::Go(route) => Some(route),
                _ => None,
            },
            Item::Card(_) => None,
        })
    };
    match kind {
        MenuKind::File => Subject::Files(
            picked
                .iter()
                .filter_map(|(index, item)| match item {
                    Item::Card(card) => Some(FileTarget {
                        path: card.path.clone(),
                        media: card.media,
                        kind: card.kind,
                        watched: card.watch.is_watched(),
                        index: *index,
                    }),
                    Item::Row(_) => None,
                })
                .collect(),
        ),
        MenuKind::Folder => Subject::Folders(routes().filter_map(Route::folder).cloned().collect()),
        MenuKind::Playlist => {
            let summaries = playlists::summaries(cx);
            let summary = |route: &Route| match route {
                Route::Playlists(Some(id)) => summaries.iter().find(|s| s.id == *id).cloned(),
                _ => None,
            };
            Subject::Playlists(routes().filter_map(summary).collect())
        }
    }
}

/// What an action does with the main window and its items.
type Run<T> = Rc<dyn Fn(&mut MainWindow, &[T], &mut Window, &mut Context<MainWindow>)>;

/// An action named `label` with `icon` that runs `run` on all of `targets`.
pub(super) fn on_all<T: Clone + 'static>(
    main: &WeakEntity<MainWindow>,
    targets: &[T],
    label: &'static str,
    icon: &'static str,
    run: impl Fn(&mut MainWindow, &[T], &mut Window, &mut Context<MainWindow>) + 'static,
) -> MenuItem {
    let (main, targets, run): (_, Vec<T>, Run<T>) = (main.clone(), targets.to_vec(), Rc::new(run));
    MenuItem::action(label, move |window, cx| {
        let run = run.clone();
        main.update(cx, |main, cx| run(main, &targets, window, cx)).ok();
    })
    .icon(icon)
}

/// An action named `label` with `icon` that runs `run` on the one item of `targets`; disabled,
/// saying why, while there are several.
pub(super) fn on_one<T: Clone + 'static>(
    main: &WeakEntity<MainWindow>,
    targets: &[T],
    label: &'static str,
    icon: &'static str,
    run: impl Fn(&mut MainWindow, &T, &mut Window, &mut Context<MainWindow>) + 'static,
) -> MenuItem {
    let several = targets.len() > 1;
    on_all(main, &targets[..targets.len().min(1)], label, icon, move |main, one, window, cx| {
        if let Some(target) = one.first() {
            run(main, target, window, cx);
        }
    })
    .disabled(several)
    .why_disabled(ONE_AT_A_TIME)
}

/// The item menu at its place, while open.
pub(super) fn render(menu: &ItemMenu) -> Option<AnyElement> {
    let (menu, position, _) = menu.as_ref()?;
    Some(popover::at_position(*position, 1, menu.clone()))
}
