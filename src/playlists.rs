//! Saved playlists (S3.5): the [`Playlists`] global that lists them, and the operations that
//! create, fill, rename, duplicate, delete and read them.
//!
//! The rows live in the database ([`crate::db::playlists`]), which only the history thread
//! touches: every operation runs there through [`History::with_database`], in the order it was
//! asked for, and answers with a [`Task`]. Each write also reads the list again, so the global
//! is current once the task finishes and windows that observe it redraw.
//!
//! - [`title`]: what a title may be, and numbering taken ones.
//! - [`thumbnail`]: checking a picked custom thumbnail.
//! - [`export`]: writing a playlist to an `.m3u8` or `.xspf` file (S3.6.1).
//! - [`import`]: reading a playlist file to save it (S3.6.2).

pub mod export;
pub mod import;
pub mod thumbnail;
pub mod title;

use std::path::PathBuf;

use futures::channel::oneshot;
use gpui::{App, Global, Task};
use rusqlite::Connection;

use crate::app::media::Media;
use crate::db::media as media_rows;
use crate::db::now_ms;
use crate::db::playlists::{self, PlaylistId, Stored, Summary};
use crate::history::History;
use crate::settings::stored_path;

/// The saved playlists, as the menus list them.
#[derive(Default)]
pub struct Playlists {
    summaries: Vec<Summary>,
}

impl Global for Playlists {}

/// What the New Playlist dialog and Save Queue as Playlist save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Draft {
    /// The title as typed; it is cleaned and numbered when taken.
    pub title: String,
    /// A custom thumbnail image.
    pub thumbnail: Option<PathBuf>,
    /// The items, in order.
    pub items: Vec<Media>,
}

/// A deleted playlist, which Undo puts back.
#[derive(Clone, Debug)]
pub struct Deleted {
    id: PlaylistId,
    stored: Stored,
}

impl Deleted {
    /// The deleted playlist's title.
    pub fn title(&self) -> &str {
        &self.stored.title
    }
}

/// Starts with no playlists and reads them from the database.
pub fn init(cx: &mut App) {
    cx.set_global(Playlists::default());
    run(cx, |_, _| Ok(())).detach();
}

/// The saved playlists, by title.
pub fn summaries(cx: &App) -> &[Summary] {
    cx.try_global::<Playlists>().map_or(&[], |playlists| &playlists.summaries)
}

/// Saves `draft` as a new playlist, and answers with the title it got. `None` when the title is
/// blank or the database failed.
pub fn create(draft: &Draft, cx: &mut App) -> Task<Option<String>> {
    let Some(wanted) = title::clean(&draft.title) else { return Task::ready(None) };
    let thumbnail = draft.thumbnail.as_deref().map(stored_path::encode);
    let locations = locations(&draft.items);
    run(cx, move |connection, now| {
        let title = free_title(connection, &wanted, None)?;
        let stored = Stored { title: title.clone(), thumbnail, locations };
        playlists::insert(connection, None, &stored, now)?;
        Ok(title)
    })
}

/// Adds `items` at the end of playlist `id`.
pub fn add(id: PlaylistId, items: &[Media], cx: &mut App) -> Task<Option<()>> {
    let locations = locations(items);
    run(cx, move |connection, now| playlists::append(connection, id, &locations, now))
}

/// Renames playlist `id`, and answers with the title it got.
pub fn rename(id: PlaylistId, raw: &str, cx: &mut App) -> Task<Option<String>> {
    let Some(wanted) = title::clean(raw) else { return Task::ready(None) };
    run(cx, move |connection, now| {
        let title = free_title(connection, &wanted, Some(id))?;
        playlists::rename(connection, id, &title, now)?;
        Ok(title)
    })
}

/// Copies playlist `id` under a numbered title, and answers with that title.
pub fn duplicate(id: PlaylistId, cx: &mut App) -> Task<Option<String>> {
    run(cx, move |connection, now| {
        let Some(mut stored) = playlists::load(connection, id)? else {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        };
        stored.title = free_title(connection, &stored.title, None)?;
        playlists::insert(connection, None, &stored, now)?;
        Ok(stored.title)
    })
}

/// Deletes playlist `id`, and answers with what Undo needs.
pub fn delete(id: PlaylistId, cx: &mut App) -> Task<Option<Deleted>> {
    run(cx, move |connection, _| {
        let stored = playlists::delete(connection, id)?;
        stored.map(|stored| Deleted { id, stored }).ok_or(rusqlite::Error::QueryReturnedNoRows)
    })
}

/// Puts a deleted playlist back where it was.
pub fn restore(deleted: Deleted, cx: &mut App) -> Task<Option<()>> {
    run(cx, move |connection, now| {
        playlists::insert(connection, Some(deleted.id), &deleted.stored, now).map(drop)
    })
}

/// Playlist `id`'s items, in order. Items whose stored text can't be read are left out.
pub fn items(id: PlaylistId, cx: &mut App) -> Task<Option<Vec<Media>>> {
    run(cx, move |connection, _| {
        let stored = playlists::load(connection, id)?.unwrap_or_default();
        Ok(stored.locations.iter().filter_map(|location| Media::from_stored(location)).collect())
    })
}

/// Playlist `id`'s items with the lengths the history knows, for exporting it.
pub fn export_items(id: PlaylistId, cx: &mut App) -> Task<Option<Vec<export::Item>>> {
    run(cx, move |connection, _| {
        let stored = playlists::load(connection, id)?.unwrap_or_default();
        let mut items = Vec::with_capacity(stored.locations.len());
        for location in &stored.locations {
            let Some(media) = Media::from_stored(location) else { continue };
            items.push(export::Item { media, duration: known_duration(connection, location)? });
        }
        Ok(items)
    })
}

/// The length the history remembers for the media at `location`.
fn known_duration(
    connection: &Connection,
    location: &str,
) -> rusqlite::Result<Option<std::time::Duration>> {
    let Some(row) = media_rows::find_at(connection, location)? else { return Ok(None) };
    Ok(media_rows::saved(connection, row.id)?.duration)
}

/// `wanted`, numbered when another playlist than `except` has it.
fn free_title(
    connection: &Connection,
    wanted: &str,
    except: Option<PlaylistId>,
) -> rusqlite::Result<String> {
    let others = playlists::list(connection)?;
    let taken = others.iter().filter(|s| Some(s.id) != except).map(|s| s.title.as_str());
    Ok(title::unique(wanted, taken))
}

fn locations(items: &[Media]) -> Vec<String> {
    items.iter().map(Media::to_stored).collect()
}

/// Runs `work` on the history thread's database, then reads the list into the global. Answers
/// `None` when the work failed (logged) or there is no database (tests).
fn run<T: Send + 'static>(
    cx: &mut App,
    work: impl FnOnce(&mut Connection, i64) -> rusqlite::Result<T> + Send + 'static,
) -> Task<Option<T>> {
    let Some(history) = History::get(cx) else { return Task::ready(None) };
    let (reply, answer) = oneshot::channel();
    history.with_database(move |connection| {
        let done = work(connection, now_ms())
            .and_then(|value| Ok((value, playlists::list(connection)?)))
            .inspect_err(|error| tracing::warn!("playlists: {error}"));
        // The app may be quitting.
        let _ = reply.send(done.ok());
    });
    cx.spawn(async move |cx| {
        let (value, summaries) = answer.await.ok().flatten()?;
        cx.update(|cx| cx.set_global(Playlists { summaries }));
        Some(value)
    })
}
