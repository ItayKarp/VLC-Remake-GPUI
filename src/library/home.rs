//! What Home lists (S4.9.1): Continue Watching, Listen Again, Favorites (S4.8.5), and the
//! folders, playlists and artists used most, read from the database in one go ([`read`]).
//!
//! Each section reads up to [`LISTED`] items, which its `>` lists in full; Home itself shows the
//! first few. With "Save recently played items" off, only Favorites is read: the rest comes from
//! what was played and visited (S4.9 edge case).
//!
//! Continue Watching's files are checked on the disk afterwards ([`Home::drop_unavailable`]):
//! those on an unplugged drive are left out and counted, so Home can say how many.
//!
//! Visits are counted, and Continue Watching items taken off, by [`writes`].

pub mod writes;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use super::display;
use super::folder_view::{Card, kind_by_extension};
use super::watch_state::WatchState;
use crate::app::media::Media;
use crate::db::activity::{self, Favorite};
use crate::db::folders;
use crate::db::metadata::MediaKind;
use crate::db::played::{self, Played};
use crate::db::playlists::PlaylistId;
use crate::history::policy::WatchedRule;
use crate::navigation::route::HomeSection;

/// The most items a section reads: what its full list shows.
pub const LISTED: usize = 100;

/// Home's sections, each in its order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Home {
    /// Videos left partway, the most recent first, whose files are there.
    pub continue_watching: Vec<Card>,
    /// How many partway videos are on a drive or share that isn't there.
    pub unavailable: usize,
    /// Audio played lately, the most recent first.
    pub listen_again: Vec<Card>,
    /// Favorite media, the newest favorite first.
    pub favorite_media: Vec<Card>,
    /// Favorite playlists, the newest favorite first.
    pub favorite_playlists: Vec<PlaylistId>,
    /// The library folders visited most.
    pub folders: Vec<PathBuf>,
    /// The saved playlists visited most.
    pub playlists: Vec<PlaylistId>,
    /// The artists played most.
    pub artists: Vec<String>,
}

/// What a read of Home needs to know.
#[derive(Clone, Copy, Debug)]
pub struct Wanted {
    /// What counts as watched.
    pub rule: WatchedRule,
    /// Whether recently played items are saved; when not, only favorites are read.
    pub history: bool,
    /// Now, in milliseconds since the epoch, for the fading of visits and plays.
    pub now_ms: i64,
}

impl Home {
    /// The cards `section` lists: its media, for the sections that have them.
    pub fn cards(&self, section: HomeSection) -> &[Card] {
        match section {
            HomeSection::ContinueWatching => &self.continue_watching,
            HomeSection::ListenAgain => &self.listen_again,
            HomeSection::Favorites => &self.favorite_media,
            HomeSection::Folders | HomeSection::Playlists | HomeSection::Artists => &[],
        }
    }

    /// Leaves out the partway videos whose files aren't there, counting them. Blocks on the
    /// disk.
    pub fn drop_unavailable(&mut self) {
        let before = self.continue_watching.len();
        self.continue_watching.retain(|card| card.path.exists());
        self.unavailable = before - self.continue_watching.len();
    }
}

/// Everything Home lists, as `wanted` says. Blocks on the database.
pub fn read(connection: &Connection, wanted: Wanted) -> rusqlite::Result<Home> {
    let (favorite_media, favorite_playlists) = read_favorites(connection, wanted.rule)?;
    let mut home = Home { favorite_media, favorite_playlists, ..Home::default() };
    if !wanted.history {
        return Ok(home);
    }
    let in_progress = played::in_progress(connection, LISTED)?;
    home.continue_watching = continue_watching(in_progress, wanted.rule);
    let recent = played::recent(connection, MediaKind::Audio, LISTED)?;
    home.listen_again = recent.into_iter().map(|played| song_card(played, wanted.rule)).collect();
    let folders = activity::frequent_folders(connection, wanted.now_ms, LISTED)?;
    for id in folders {
        home.folders.extend(folders::path(connection, id)?);
    }
    home.playlists = activity::frequent_playlists(connection, wanted.now_ms, LISTED)?;
    home.artists = activity::frequent_artists(connection, wanted.now_ms, LISTED)?;
    Ok(home)
}

/// The cards of the `in_progress` videos that `rule` still counts as partway: a position the
/// rule now calls watched (its setting was lowered) is left out.
fn continue_watching(in_progress: Vec<Played>, rule: WatchedRule) -> Vec<Card> {
    in_progress
        .into_iter()
        .map(|played| {
            let mut card = card(played, rule);
            card.label = Media::File(card.path.clone()).display_name();
            card
        })
        .filter(|card| card.kind != MediaKind::Audio)
        .filter(|card| matches!(card.watch, WatchState::InProgress(_)))
        .collect()
}

/// The favorite media and playlists, the newest favorite first.
fn read_favorites(
    connection: &Connection,
    rule: WatchedRule,
) -> rusqlite::Result<(Vec<Card>, Vec<PlaylistId>)> {
    let (mut media, mut playlists) = (Vec::new(), Vec::new());
    for favorite in activity::favorites(connection)?.into_iter().rev() {
        match favorite {
            Favorite::Media(id) => {
                media.extend(played::one(connection, id)?.map(|p| card(p, rule)));
            }
            Favorite::Playlist(id) => playlists.push(id),
        }
    }
    Ok((media, playlists))
}

/// A song's card, saying its title and artist (`untangle - juxtopposed`).
fn song_card(played: Played, rule: WatchedRule) -> Card {
    let mut card = card(played, rule);
    if let Some(artist) = card.tags.artist.as_ref().or(card.tags.album_artist.as_ref()) {
        card.label = format!("{} - {artist}", card.label);
    }
    card
}

/// The card of a played media, titled as its tags say.
fn card(played: Played, rule: WatchedRule) -> Card {
    let probed = played.metadata.kind;
    let kind = if probed == MediaKind::Unknown { kind_by_extension(&played.path) } else { probed };
    Card {
        label: display::title(&played.metadata, Path::new(&played.path)),
        media: Some(played.media),
        kind,
        watch: WatchState::of(&played.saved, rule),
        path: played.path,
        tags: played.metadata,
        added_ms: played.added_ms,
    }
}

#[cfg(test)]
mod tests;
