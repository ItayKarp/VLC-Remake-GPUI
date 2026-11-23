//! Subtitle files linked to media by the Add Subtitles dialog (S6.4): which ones each media has,
//! and saving what the dialog changed.
//!
//! A link is a row in [`crate::db::subtitle_links`] (D16: linking is the default, as it touches
//! neither file); "Copy next to video" copies the subtitle beside the video first ([`copy`]) and
//! links the copy. Linked files load with the media whenever it opens, like the ones found
//! beside it. The rows live in the database, which only the history thread touches.
//!
//! - [`episode`] and [`pairing`]: pairing a folder's videos with subtitle files.
//! - [`copy`]: copying a subtitle beside its video.
//! - [`download`]: From Link.
//! - [`query`] and [`opensubtitles`]: From OpenSubtitles.

pub mod copy;
pub mod download;
pub mod episode;
pub mod opensubtitles;
pub mod pairing;
pub mod query;

use std::fs;
use std::path::{Path, PathBuf};

use futures::channel::oneshot;
use gpui::{App, AppContext as _, Task};

use self::copy::{Copied, OnConflict};
use crate::db::subtitle_links;
use crate::history::History;
use crate::opening::kind::{self, Kind};
use crate::settings::stored_path;

/// A subtitle to link to a video.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Addition {
    /// The subtitle file.
    pub subtitle: PathBuf,
    /// Copy it next to the video first, deciding a taken name this way; `None` links it where
    /// it is.
    pub copy: Option<OnConflict>,
}

/// What the dialog changed for one video.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The video.
    pub video: PathBuf,
    /// Subtitles to link.
    pub add: Vec<Addition>,
    /// Linked subtitles to unlink.
    pub remove: Vec<PathBuf>,
}

/// What saving one addition did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The video.
    pub video: PathBuf,
    /// The subtitle as the dialog named it.
    pub subtitle: PathBuf,
    /// The file now linked (the copy, when one was made), or why nothing was.
    pub linked: Result<PathBuf, String>,
}

/// The subtitle files linked to the video at `media`, oldest link first; empty when the
/// database can't be read.
pub fn linked(media: &Path, cx: &App) -> Task<Vec<PathBuf>> {
    let task = linked_to_each(vec![media.to_path_buf()], cx);
    cx.background_spawn(async move { task.await.into_iter().next().unwrap_or_default() })
}

/// The subtitle files linked to each of `videos`, in their order, in one database visit.
pub fn linked_to_each(videos: Vec<PathBuf>, cx: &App) -> Task<Vec<Vec<PathBuf>>> {
    let count = videos.len();
    let Some(history) = History::get(cx) else { return Task::ready(vec![Vec::new(); count]) };
    let rows = history.query("subtitle links", move |connection| {
        let load = |video: &PathBuf| subtitle_links::load(connection, &stored_path::encode(video));
        videos.iter().map(load).collect::<rusqlite::Result<Vec<_>>>()
    });
    cx.background_spawn(async move {
        let rows = rows.await.ok().flatten().unwrap_or_else(|| vec![Vec::new(); count]);
        rows.iter()
            .map(|rows| rows.iter().filter_map(|row| stored_path::decode(row)).collect())
            .collect()
    })
}

/// Saves `changes`: copies what is to be copied, then links and unlinks in the database.
/// Answers what each addition did, once the database has it.
pub fn save(changes: Vec<Change>, cx: &App) -> Task<Vec<Outcome>> {
    let history = History::get(cx).map(History::database);
    cx.background_spawn(async move {
        let outcomes: Vec<Outcome> = changes.iter().flat_map(prepare).collect();
        let rows = rows_to_write(&changes, &outcomes);
        let Some(history) = history else { return outcomes };
        let (reply, written) = oneshot::channel();
        history.run(Box::new(move |connection| {
            let _ = reply.send(write(connection, &rows));
        }));
        match written.await {
            Ok(Ok(())) => outcomes,
            Ok(Err(error)) => failed_all(outcomes, &format!("Couldn't save the link: {error}.")),
            Err(_) => failed_all(outcomes, "Couldn't save the link."),
        }
    })
}

/// The media files in `folder`, not its subfolders, in name order: a folder target's videos.
/// Reads the folder, so it belongs off the UI thread.
pub fn media_in(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let hidden = path.file_name().is_none_or(|n| n.to_string_lossy().starts_with('.'));
            !hidden && kind::by_extension(path) == Some(Kind::Media) && path.is_file()
        })
        .collect();
    found.sort();
    found
}

/// Copies each of `change`'s additions that asks for it, answering the file to link for each.
fn prepare(change: &Change) -> Vec<Outcome> {
    let video_there = change.video.is_file();
    change
        .add
        .iter()
        .map(|addition| {
            let linked = if video_there {
                link_target(&change.video, addition)
            } else {
                Err("The video is gone.".to_owned())
            };
            Outcome { video: change.video.clone(), subtitle: addition.subtitle.clone(), linked }
        })
        .collect()
}

/// The file to link for `addition` to `video`: the copy, when one is asked for and made.
fn link_target(video: &Path, addition: &Addition) -> Result<PathBuf, String> {
    if !addition.subtitle.is_file() {
        return Err("The subtitle file is gone.".to_owned());
    }
    let Some(on_conflict) = addition.copy else { return Ok(addition.subtitle.clone()) };
    match copy::copy(video, &addition.subtitle, on_conflict) {
        Ok(Copied::To(copied)) => Ok(copied),
        Ok(Copied::AlreadyThere | Copied::Skipped) => Ok(addition.subtitle.clone()),
        Err(error) => Err(copy::failure_message(&error)),
    }
}

/// A database row to write: link (`true`) or unlink a subtitle, by stored locations.
type Row = (String, String, bool);

/// The rows `changes` write, given what their additions did.
fn rows_to_write(changes: &[Change], outcomes: &[Outcome]) -> Vec<Row> {
    let encode = |path: &Path| stored_path::encode(path);
    let links = outcomes.iter().filter_map(|outcome| {
        let linked = outcome.linked.as_ref().ok()?;
        Some((encode(&outcome.video), encode(linked), true))
    });
    let unlinks = changes.iter().flat_map(|change| {
        change.remove.iter().map(move |subtitle| (encode(&change.video), encode(subtitle), false))
    });
    unlinks.chain(links).collect()
}

fn write(connection: &mut rusqlite::Connection, rows: &[Row]) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    for (media, subtitle, link) in rows {
        if *link {
            subtitle_links::link(&transaction, media, subtitle)?;
        } else {
            subtitle_links::unlink(&transaction, media, subtitle)?;
        }
    }
    transaction.commit()
}

/// `outcomes` with every link that was made marked failed with `why`.
fn failed_all(outcomes: Vec<Outcome>, why: &str) -> Vec<Outcome> {
    outcomes
        .into_iter()
        .map(|outcome| Outcome { linked: outcome.linked.and(Err(why.to_owned())), ..outcome })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    #[test]
    fn a_gone_video_or_subtitle_fails_its_row() {
        let folder = scratch_dir();
        let (video, subtitle) = (folder.join("Movie.mkv"), folder.join("Movie.en.srt"));
        fs::write(&subtitle, "1\n00:00:01,000 --> 00:00:02,000\nHi\n").unwrap();
        let addition = Addition { subtitle: subtitle.clone(), copy: None };
        let change = Change { video: video.clone(), add: vec![addition], remove: Vec::new() };
        assert_eq!(prepare(&change)[0].linked, Err("The video is gone.".to_owned()));

        fs::write(&video, b"video").unwrap();
        assert_eq!(prepare(&change)[0].linked, Ok(subtitle));
        let gone = Addition { subtitle: folder.join("gone.srt"), copy: None };
        let change = Change { add: vec![gone], ..change };
        assert!(prepare(&change)[0].linked.is_err());
    }

    #[test]
    fn only_media_files_are_a_folders_videos() {
        let folder = scratch_dir();
        for name in ["b.mkv", "a.mp4", "a.srt", ".hidden.mkv", "notes.txt"] {
            fs::write(folder.join(name), b"x").unwrap();
        }
        let names: Vec<_> = media_in(&folder)
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.mp4", "b.mkv"]);
    }
}
