//! What the sidebar lists for its route (S4.5, S4.6): the places a route leads to (media folders,
//! subfolders, Music's categories, playlists), the recently played media (History), in a
//! Videos or Songs folder its media as cards ([`card`](super::card)), and in Albums, Artists
//! and Genres their names, then one name's songs ([`music`]), Network its categories and
//! streams ([`network`]), and a search what it finds ([`search`]).
//!
//! A folder is read in the background: its subfolders and media files from the disk, and what
//! the library knows of them from the database ([`folder_view`]). A folder that is gone says
//! "This folder is no longer available" with Back (S4.5 edge case). The rows are virtualized, so
//! a folder of 10,000 files draws only what is on screen.

mod music;
mod network;
mod search;

use std::path::PathBuf;

use futures::channel::oneshot;
use gpui::{App, Context, Task, prelude::*};

use super::MainWindow;
use super::row::{Row, Target};
use super::selection::Key;
use crate::app::media::Media;
use crate::assets::icons;
use crate::db::metadata::MediaKind;
use crate::history::History;
use crate::history::policy::WatchedRule;
use crate::library::folder_view::{self, Card, OnDisk};
use crate::library::sort;
use crate::navigation::route::{MusicCategory, MusicRoute, Route};
use crate::navigation::sort::Sort;
use crate::opening::recent;
use crate::playlists;
use crate::settings;

/// What a route that reads the disk or the library lists, once read: a folder, or a place in
/// Albums, Artists or Genres.
pub(in crate::main_window) struct FolderListing {
    route: Route,
    state: FolderState,
    _read: Task<()>,
}

enum FolderState {
    Reading,
    Read(Contents),
    Gone,
}

/// A folder's subfolders, or a category's names, then the cards of the media, each in order.
#[derive(Default)]
struct Contents {
    folders: Vec<PathBuf>,
    names: Vec<String>,
    cards: Vec<Card>,
}

impl Contents {
    /// Orders the subfolders and cards by `sort`: subfolders by name, Z to A only when `sort`
    /// orders names that way.
    fn sort(&mut self, sort: Sort) {
        folder_view::sort_folders(&mut self.folders);
        if sort.reverses_names() {
            self.folders.reverse();
        }
        sort::sort_cards(&mut self.cards, sort);
    }
}

/// What the body lists: the rows and the cards, in [`Order`].
pub(super) struct Items<'a> {
    pub(super) rows: Vec<Row>,
    pub(super) cards: &'a [Card],
    pub(super) order: Order,
}

/// Which come first: Home lists its media before its places; everything else the other way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Order {
    RowsFirst,
    CardsFirst,
}

/// One listed item.
pub(super) enum Item<'a> {
    Row(&'a Row),
    Card(&'a Card),
}

impl<'a> Items<'a> {
    /// The rows, then the cards.
    pub(super) fn new(rows: Vec<Row>, cards: &'a [Card]) -> Self {
        Self { rows, cards, order: Order::RowsFirst }
    }
}

impl Items<'_> {
    /// How many items there are.
    pub(super) fn len(&self) -> usize {
        self.rows.len() + self.cards.len()
    }

    /// The item at `index`.
    pub(super) fn get(&self, index: usize) -> Option<Item<'_>> {
        match self.order {
            Order::RowsFirst => match self.rows.get(index) {
                Some(row) => Some(Item::Row(row)),
                None => self.cards.get(index - self.rows.len()).map(Item::Card),
            },
            Order::CardsFirst => match self.cards.get(index) {
                Some(card) => Some(Item::Card(card)),
                None => self.rows.get(index - self.cards.len()).map(Item::Row),
            },
        }
    }

    /// Where item `index` is among the cards, if it is one.
    pub(super) fn card_index(&self, index: usize) -> Option<usize> {
        let first = match self.order {
            Order::RowsFirst => self.rows.len(),
            Order::CardsFirst => 0,
        };
        index.checked_sub(first).filter(|&at| at < self.cards.len())
    }

    /// What each item is, in order, for the selection.
    pub(super) fn keys(&self) -> Vec<Key> {
        let rows = self.rows.iter().map(Row::key);
        let cards = self.cards.iter().map(card_key);
        match self.order {
            Order::RowsFirst => rows.chain(cards).collect(),
            Order::CardsFirst => cards.chain(rows).collect(),
        }
    }

    /// How many items the header counts: action rows ("+ Add") aren't items (S4.5.5).
    pub(super) fn counted(&self) -> usize {
        self.rows.iter().filter(|row| !row.is_action()).count() + self.cards.len()
    }

    /// Every card's media, in order: what playing one of them queues.
    pub(super) fn card_media(&self) -> Vec<Media> {
        self.cards.iter().map(|card| Media::File(card.path.clone())).collect()
    }
}

/// What a card is, for the selection: its media when the library knows it, else its file.
pub(super) fn card_key(card: &Card) -> Key {
    card.media.map_or_else(|| Key::File(card.path.clone()), Key::Media)
}

/// What the body shows.
pub(super) enum Body<'a> {
    Items(Items<'a>),
    /// The folder is still being read.
    Reading,
    /// The route's folder or playlist is gone.
    Gone(&'static str),
}

impl MainWindow {
    /// Reads the current route's folder or music again, if it shows one and the sidebar is
    /// open.
    pub(super) fn reload_folder(&mut self, cx: &mut Context<Self>) {
        self.reload_home(cx);
        let route = self.library_panel.history.current().clone();
        let sort = self.library_sort(cx);
        let read = if !self.library_panel.is_open() {
            None
        } else if let Some(folder) = route.folder() {
            let sort = sort.unwrap_or(Sort::TitleAscending);
            Some(read_folder(route.clone(), folder.clone(), sort, cx))
        } else {
            music::read(route.clone(), sort, cx).or_else(|| search::read(route.clone(), cx))
        };
        let Some(read) = read else {
            self.library_panel.folder = None;
            return;
        };
        // The same place read again keeps its rows meanwhile, so they don't flicker.
        let state = match self.library_panel.folder.take() {
            Some(old) if old.route == route => old.state,
            _ => FolderState::Reading,
        };
        self.library_panel.folder = Some(FolderListing { route, state, _read: read });
    }

    /// Shows what was read for `route`, if the listing still shows it, selecting the item Go
    /// to Source asked for.
    fn read_into(&mut self, route: &Route, state: FolderState, cx: &mut Context<Self>) {
        let listing = self.library_panel.folder.as_mut();
        if let Some(listing) = listing.filter(|listing| listing.route == *route) {
            listing.state = state;
            self.select_revealed(cx);
            cx.notify();
        }
    }

    /// Selects and scrolls to the item Go to Source asked for, once the listing is read.
    pub(super) fn select_revealed(&mut self, cx: &App) {
        let Body::Items(items) = self.listing(cx) else { return };
        let keys = items.keys();
        let Some(key) = self.library_panel.reveal.take() else { return };
        let Some(index) = keys.iter().position(|listed| *listed == key) else { return };
        self.library_panel.selection.select(key);
        self.scroll_listing_to(index, gpui::ScrollStrategy::Center, cx);
    }

    /// Orders what the current route lists by `sort`, if it was read.
    pub(super) fn sort_listing(&mut self, sort: Sort) {
        let listing = self.library_panel.folder.as_mut();
        if let Some(FolderListing { state: FolderState::Read(contents), .. }) = listing {
            contents.sort(sort);
        }
    }

    /// What the current route lists, or why it lists nothing.
    pub(super) fn listing(&self, cx: &App) -> Body<'_> {
        let route = self.library_panel.history.current();
        if let Some(body) = self.home_listing(route, cx) {
            return body;
        }
        if let Some(listing) = &self.library_panel.folder {
            return match &listing.state {
                FolderState::Reading => Body::Reading,
                FolderState::Read(contents) => {
                    Body::Items(Items::new(read_rows(route, contents), &contents.cards))
                }
                FolderState::Gone => Body::Gone(tr!("This folder is no longer available.")),
            };
        }
        if !super::exists(route, cx) {
            return Body::Gone(tr!("This playlist no longer exists."));
        }
        Body::Items(Items::new(rows(route, cx), &[]))
    }

    /// How many items the route lists, for the header's count.
    pub(super) fn listed_count(&self, cx: &App) -> usize {
        match self.listing(cx) {
            Body::Items(items) => items.counted(),
            Body::Reading | Body::Gone(_) => 0,
        }
    }
}

/// The rows of what was read for `route`: a folder's subfolders, or a category's names.
fn read_rows(route: &Route, contents: &Contents) -> Vec<Row> {
    if route.folder().is_some() {
        folder_rows(route, &contents.folders, true)
    } else {
        music::name_rows(route, &contents.names)
    }
}

/// The kind of media a folder route lists: Videos lists video, Songs audio.
fn section_kind(route: &Route) -> MediaKind {
    match route {
        Route::Music(_) => MediaKind::Audio,
        _ => MediaKind::Video,
    }
}

/// Reads `folder` in the background into the listing, if it still shows `route`: the disk and
/// the library at once, then the cards of the route's kind, ordered by `sort`.
fn read_folder(
    route: Route,
    folder: PathBuf,
    sort: Sort,
    cx: &mut Context<MainWindow>,
) -> Task<()> {
    let wanted = section_kind(&route);
    let prefs = &settings::get(cx).prefs;
    let show_hidden = prefs.library.show_hidden_files;
    let rule = WatchedRule::from_settings(&prefs.player);
    let known = read_library(folder.clone(), cx);
    let on_disk = cx.background_spawn(async move { folder_view::read_disk(&folder, show_hidden) });
    cx.spawn(async move |this, cx| {
        let state = match on_disk.await {
            Ok(OnDisk { folders, files }) => {
                let known = known.await.unwrap_or_default();
                let cards = cx.background_spawn(async move {
                    let cards = folder_view::cards(files, known, wanted, rule);
                    let mut contents = Contents { folders, cards, ..Contents::default() };
                    contents.sort(sort);
                    contents
                });
                FolderState::Read(cards.await)
            }
            Err(_) => FolderState::Gone,
        };
        this.update(cx, |this, cx| this.read_into(&route, state, cx)).ok();
    })
}

/// What the library knows of `folder`'s files, read on the history thread.
fn read_library(folder: PathBuf, cx: &App) -> oneshot::Receiver<Vec<crate::db::files::Listed>> {
    let (reply, answer) = oneshot::channel();
    if let Some(history) = History::get(cx) {
        history.with_database(move |connection| {
            let known = folder_view::read_library(connection, &folder).unwrap_or_else(|error| {
                tracing::warn!("library: can't read a folder's files: {error}");
                Vec::new()
            });
            let _ = reply.send(known);
        });
    }
    answer
}

/// The rows a route that shows no folder lists.
fn rows(route: &Route, cx: &App) -> Vec<Row> {
    let local = &settings::get(cx).local;
    match route {
        Route::Videos(None) => folder_rows(route, &local.media_folders, false),
        Route::Music(MusicRoute::Songs(None)) => folder_rows(route, &local.music_folders, false),
        Route::Music(MusicRoute::Categories) => MusicCategory::ALL
            .into_iter()
            .map(|category| {
                Row::new(category.label(), category_icon(category), Target::Go(category.route()))
            })
            .collect(),
        Route::Playlists(None) => playlists::summaries(cx)
            .iter()
            .map(|summary| {
                let target = Target::Go(Route::Playlists(Some(summary.id)));
                Row::new(summary.title.clone(), icons::PLAYLIST, target)
            })
            .collect(),
        Route::History => recent::items(cx).into_iter().map(Row::recent).collect(),
        Route::Network(category) => network::rows(*category, cx),
        Route::Podcast(id) => network::episode_rows(*id, cx),
        _ => Vec::new(),
    }
}

/// The rows of `folders` in a Videos or Songs route, after "+ Add". A folder's own level has
/// "+ Add" only when it has subfolders: the designs show none in a folder of files only
/// (`sidebar-default`). The media folders' list always has it.
fn folder_rows(route: &Route, folders: &[PathBuf], in_folder: bool) -> Vec<Row> {
    let add = (!in_folder || !folders.is_empty()).then(|| Row::add(route)).flatten();
    let row = |folder: &PathBuf| {
        let target = route.with_folder(folder.clone())?;
        let label = Media::File(folder.clone()).display_name();
        Some(Row::new(label, icons::FOLDER, Target::Go(target)))
    };
    add.into_iter().chain(folders.iter().filter_map(row)).collect()
}

/// The icon of `category`'s rows.
fn category_icon(category: MusicCategory) -> &'static str {
    match category {
        MusicCategory::Songs => icons::MUSIC,
        MusicCategory::Albums => icons::ALBUM,
        MusicCategory::Artists => icons::ARTIST,
        MusicCategory::Genres => icons::GENRE,
    }
}
