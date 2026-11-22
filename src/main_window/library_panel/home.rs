//! Home in the listing (S4.9; `sidebar-home`, `player_07`): the first few items of each section
//! under its heading and `>`, or in full library mode a line of tiles (S4.10.2) ([`page`]),
//! each section's full list behind its `>`, and a welcome when there is nothing yet
//! ([`welcome`]).
//!
//! Home is read from the library in the background ([`crate::library::home`]) whenever the
//! listing is, and kept while it is read again so it doesn't flicker. Its items are the
//! listing's own, so they select, open, drag and have menus like anywhere else; its media come
//! first, then its places, which is the order its sections show them in. A media opened on Home
//! plays alone: the others beside it aren't related.

mod page;
mod welcome;

use std::path::PathBuf;

use futures::channel::oneshot;
use gpui::{App, AppContext as _, Context, Task, WeakEntity};

use super::MainWindow;
use super::item_menu::{FileTarget, on_all};
use super::layout::Shape;
use super::listing::{Body, Items, Order};
use super::row::{Row, Target};
use crate::app::media::Media;
use crate::assets::icons;
use crate::db::now_ms;
use crate::db::playlists::PlaylistId;
use crate::history::History;
use crate::history::policy::WatchedRule;
use crate::library::folder_view::Card;
use crate::library::home::{self, Home, Wanted, writes};
use crate::navigation::item_count;
use crate::navigation::route::{HomeSection, MusicRoute, Route};
use crate::navigation::view::View;
use crate::playlists;
use crate::settings::{self, Settings};
use crate::ui::menu::MenuItem;

/// How many items each section shows on Home itself in the sidebar (`sidebar-home`); full
/// library mode shows a line of tiles (`player_07`).
const SHOWN: usize = 3;

/// Home as last read, and the read under way.
pub(super) struct HomeListing {
    read: Option<Read>,
    _task: Task<()>,
}

/// What was read, and the page made of it.
struct Read {
    home: Home,
    page: Page,
}

/// What Home's page shows: each section's first few items, its media before its places.
struct Page {
    cards: Vec<Card>,
    rows: Vec<Row>,
    sections: Vec<Shown>,
    /// Whether recently played items are saved; when not, the page says why most sections
    /// are missing.
    history: bool,
    /// How many of Continue Watching's videos are away.
    unavailable: usize,
    /// The most items a section shows.
    limit: usize,
}

/// One section on the page, and how many of its media and places it shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shown {
    section: HomeSection,
    cards: usize,
    rows: usize,
}

impl Page {
    /// The page of `home`: sections with nothing are left out, except Continue Watching while
    /// some of it is away, which says so.
    fn of(home: &Home, history: bool, limit: usize, cx: &App) -> Self {
        let mut page = Self {
            cards: Vec::new(),
            rows: Vec::new(),
            sections: Vec::new(),
            history,
            unavailable: home.unavailable,
            limit,
        };
        for section in HomeSection::ALL {
            let cards = &home.cards(section)[..home.cards(section).len().min(limit)];
            let rows: Vec<Row> =
                section_rows(section, home, cx).into_iter().take(limit - cards.len()).collect();
            let away = section == HomeSection::ContinueWatching && home.unavailable > 0;
            if cards.is_empty() && rows.is_empty() && !away {
                continue;
            }
            page.sections.push(Shown { section, cards: cards.len(), rows: rows.len() });
            page.cards.extend_from_slice(cards);
            page.rows.extend(rows);
        }
        page
    }

    /// What `section` says under its items about those that are away, if anything.
    fn away_note(&self, section: HomeSection) -> Option<String> {
        let away = section == HomeSection::ContinueWatching && self.unavailable > 0;
        away.then(|| format!("{} unavailable", item_count(self.unavailable)))
    }

    /// Which of the page's children (headings, items or `lined` sections' lines of tiles, and
    /// notes, top to bottom) item `index` is.
    fn child_of(&self, index: usize, lined: bool) -> Option<usize> {
        let (mut child, mut card, mut row) = (0, 0, self.cards.len());
        for shown in &self.sections {
            child += 1;
            let items = usize::from(!lined);
            if (card..card + shown.cards).contains(&index) {
                return Some(child + (index - card) * items);
            }
            if (row..row + shown.rows).contains(&index) {
                return Some(child + (shown.cards + index - row) * items);
            }
            let drawn = if lined { 1 } else { shown.cards + shown.rows };
            child += drawn + usize::from(self.away_note(shown.section).is_some());
            (card, row) = (card + shown.cards, row + shown.rows);
        }
        None
    }
}

impl MainWindow {
    /// Reads Home again in the background, if the sidebar shows it or one of its sections.
    pub(super) fn reload_home(&mut self, cx: &mut Context<Self>) {
        let route = self.library_panel.history.current();
        if !self.library_panel.is_open() || !matches!(route, Route::Home | Route::HomeSection(_)) {
            return;
        }
        let prefs = &settings::get(cx).prefs;
        let wanted = Wanted {
            rule: WatchedRule::from_settings(&prefs.player),
            history: prefs.privacy.save_recently_played,
            now_ms: now_ms(),
        };
        let answer = read_home(wanted, cx);
        let task = cx.spawn(async move |this, cx| {
            let mut home = answer.await.unwrap_or_default();
            let home = cx.background_spawn(async move {
                home.drop_unavailable();
                home
            });
            let home = home.await;
            this.update(cx, |this, cx| this.home_read(home, wanted.history, cx)).ok();
        });
        let read = self.library_panel.home.take().and_then(|listing| listing.read);
        self.library_panel.home = Some(HomeListing { read, _task: task });
    }

    /// Shows what was read, and asks for the page's thumbnails.
    fn home_read(&mut self, home: Home, history: bool, cx: &mut Context<Self>) {
        let limit = self.home_limit();
        let Some(listing) = self.library_panel.home.as_mut() else { return };
        let page = Page::of(&home, history, limit, cx);
        let on_screen = page.cards.iter().filter_map(|card| card.media).collect();
        listing.read = Some(Read { home, page });
        self.show_thumbnails(on_screen, cx);
        self.select_revealed(cx);
        cx.notify();
    }

    /// Home's page, once read, while the sidebar is on Home.
    fn home_page(&self) -> Option<&Page> {
        if *self.library_panel.history.current() != Route::Home {
            return None;
        }
        Some(&self.library_panel.home.as_ref()?.read.as_ref()?.page)
    }

    /// Which of Home's page's children item `index` is, while the sidebar is on Home.
    pub(super) fn home_child(&self, index: usize) -> Option<usize> {
        self.home_page()?.child_of(index, self.library_panel.is_full())
    }

    /// The most items a section of Home shows: a few in the sidebar, a line of video tiles in
    /// full library mode.
    fn home_limit(&self) -> usize {
        if self.library_panel.is_full() { self.home_tiles().columns } else { SHOWN }
    }

    /// How full library mode lays out Home's video tiles: as the grid's.
    fn home_tiles(&self) -> Shape {
        self.library_panel.shape(View::Grid)
    }

    /// Makes Home's page again when the window's width or the presentation changes how many
    /// items its sections show.
    pub(super) fn refit_home(&mut self, cx: &App) {
        let limit = self.home_limit();
        let Some(listing) = self.library_panel.home.as_mut() else { return };
        let Some(read) = listing.read.as_mut().filter(|read| read.page.limit != limit) else {
            return;
        };
        read.page = Page::of(&read.home, read.page.history, limit, cx);
    }

    /// What Home or one of its sections lists; `None` for other routes.
    pub(super) fn home_listing(&self, route: &Route, cx: &App) -> Option<Body<'_>> {
        if !matches!(route, Route::Home | Route::HomeSection(_)) {
            return None;
        }
        let Some(read) = self.library_panel.home.as_ref().and_then(|home| home.read.as_ref())
        else {
            return Some(Body::Reading);
        };
        let items = match route {
            Route::HomeSection(section) => {
                Items::new(section_rows(*section, &read.home, cx), read.home.cards(*section))
            }
            _ => Items {
                rows: read.page.rows.clone(),
                cards: &read.page.cards,
                order: Order::CardsFirst,
            },
        };
        Some(Body::Items(items))
    }

    /// "Remove from Continue Watching", when every one of `files` is on it.
    pub(super) fn continue_watching_item(
        &self,
        main: &WeakEntity<Self>,
        files: &[FileTarget],
    ) -> Option<MenuItem> {
        let home = &self.library_panel.home.as_ref()?.read.as_ref()?.home;
        let listed = |file: &FileTarget| {
            file.media.is_some_and(|media| {
                home.continue_watching.iter().any(|card| card.media == Some(media))
            })
        };
        if files.is_empty() || !files.iter().all(listed) {
            return None;
        }
        let label = "Remove from Continue Watching";
        Some(on_all(main, files, label, icons::REMOVE, |main, files, _, cx| {
            let media = files.iter().filter_map(|file| file.media).collect();
            writes::remove_from_continue_watching(media, cx);
            main.reload_home(cx);
        }))
    }
}

/// Home as `wanted` says, read on the history thread; nothing without the history service.
fn read_home(wanted: Wanted, cx: &App) -> oneshot::Receiver<Home> {
    let (reply, answer) = oneshot::channel();
    if let Some(history) = History::get(cx) {
        history.with_database(move |connection| {
            let home = home::read(connection, wanted).unwrap_or_else(|error| {
                tracing::warn!("library: can't read Home: {error}");
                Home::default()
            });
            let _ = reply.send(home);
        });
    }
    answer
}

/// The places `section` lists, in order: favorite playlists, folders, playlists or artists.
fn section_rows(section: HomeSection, home: &Home, cx: &App) -> Vec<Row> {
    match section {
        HomeSection::Favorites => playlist_rows(&home.favorite_playlists, cx),
        HomeSection::Folders => folder_rows(&home.folders, settings::get(cx)),
        HomeSection::Playlists => playlist_rows(&home.playlists, cx),
        HomeSection::Artists => home
            .artists
            .iter()
            .map(|name| {
                let route = Route::Music(MusicRoute::Artists(Some(name.clone())));
                Row::new(name.clone(), icons::ARTIST, Target::Go(route))
            })
            .collect(),
        HomeSection::ContinueWatching | HomeSection::ListenAgain => Vec::new(),
    }
}

/// The rows of the playlists `ids` names that still exist.
fn playlist_rows(ids: &[PlaylistId], cx: &App) -> Vec<Row> {
    let summaries = playlists::summaries(cx);
    let row = |id: &PlaylistId| {
        let summary = summaries.iter().find(|summary| summary.id == *id)?;
        let target = Target::Go(Route::Playlists(Some(*id)));
        Some(Row::new(summary.title.clone(), icons::PLAYLIST, target))
    };
    ids.iter().filter_map(row).collect()
}

/// The rows of `folders`, each opening in Songs when only a music folder holds it, else in
/// Videos.
fn folder_rows(folders: &[PathBuf], settings: &Settings) -> Vec<Row> {
    let local = &settings.local;
    let under = |roots: &[PathBuf], folder: &PathBuf| roots.iter().any(|r| folder.starts_with(r));
    let row = |folder: &PathBuf| {
        let route = if under(&local.music_folders, folder) && !under(&local.media_folders, folder) {
            Route::Music(MusicRoute::Songs(Some(folder.clone())))
        } else {
            Route::Videos(Some(folder.clone()))
        };
        let label = Media::File(folder.clone()).display_name();
        Row::new(label, icons::FOLDER, Target::Go(route))
    };
    folders.iter().map(row).collect()
}

#[cfg(test)]
mod tests;
