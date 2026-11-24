//! The playing media's bookmarks (S8.3): places the user marks in it, listed on the Bookmarks tab
//! and added there with "+" or with Cmd+B (Ctrl+B elsewhere).
//!
//! They are kept per media identity (S1.7.2) in the database's `bookmarks` table, so a renamed
//! or moved file keeps them. When a media opens, its identity is read on the history thread
//! (without writing a history row: bookmarks are the user's, not history, so they are kept
//! whatever the privacy settings say and Clear History leaves them) and its bookmarks loaded.
//! Changes show at once and are written behind.
//!
//! A new bookmark is named "<title> N" after the title tag or the file's name. Only media with a
//! length that can seek takes bookmarks, so never a live stream. A media has one bookmark per
//! millisecond: adding one where there is one already picks that one. A bookmark past the end
//! (the file was re-encoded shorter) is kept and listed, but going to it says why it can't.

use std::time::Duration;

use gpui::{App, Context};

use super::naming::{media_title, next_name, slot, to_millisecond};
use crate::app::media::Media;
use crate::db::bookmarks::{self, Bookmark};
use crate::engine::values::SeekTarget;
use crate::history::History;
use crate::history::resolve::identity_of;
use crate::main_window::MainWindow;

/// The playing media's bookmarks.
#[derive(Default)]
pub(in crate::main_window) struct Bookmarks {
    loaded: Option<Loaded>,
}

/// The bookmarks of one media.
struct Loaded {
    media: Media,
    identity: Identity,
    /// By position.
    list: Vec<Bookmark>,
}

/// Where a media's bookmarks are stored.
enum Identity {
    /// It is being read, with the bookmarks.
    Reading,
    /// They are stored under this.
    Known(String),
    /// It couldn't be read (the file is gone), so nothing can be stored.
    Unknown,
}

/// Whether the playing media can take bookmarks, for the Bookmarks tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Bookmarkable {
    /// Nothing is loaded.
    NothingLoaded,
    /// A live stream, or media that can't seek.
    CantSeek,
    /// Its bookmarks are being read.
    Reading,
    /// They can't be stored (the file can't be read).
    Unstorable,
    /// Bookmarks can be added.
    Yes,
}

impl MainWindow {
    /// Reads the bookmarks of what the player opens (a player event), unless they are read.
    pub(in crate::main_window) fn load_bookmarks(&mut self, cx: &mut Context<Self>) {
        let Some(media) = self.player_media(cx) else { return };
        if self
            .queue_panel
            .saved_bookmarks
            .loaded
            .as_ref()
            .is_some_and(|loaded| loaded.media == media)
        {
            return;
        }
        let identity =
            if History::get(cx).is_some() { Identity::Reading } else { Identity::Unknown };
        let loaded = Loaded { media: media.clone(), identity, list: Vec::new() };
        self.queue_panel.saved_bookmarks.loaded = Some(loaded);
        let Some(history) = History::get(cx) else { return };
        let asked = media.clone();
        let answer = history.query("bookmarks", move |connection| {
            let Ok(identity) = identity_of(connection, &asked) else { return Ok(None) };
            let list = bookmarks::load(connection, &identity)?;
            Ok(Some((identity, list)))
        });
        cx.spawn(async move |this, cx| {
            let found = answer.await.ok().flatten().flatten();
            this.update(cx, |this, cx| this.bookmarks_read(&media, found, cx)).ok();
        })
        .detach();
    }

    fn bookmarks_read(
        &mut self,
        media: &Media,
        found: Option<(String, Vec<Bookmark>)>,
        cx: &mut Context<Self>,
    ) {
        let Some(loaded) =
            self.queue_panel.saved_bookmarks.loaded.as_mut().filter(|l| &l.media == media)
        else {
            return;
        };
        let Some((identity, list)) = found else {
            loaded.identity = Identity::Unknown;
            return;
        };
        loaded.identity = Identity::Known(identity);
        loaded.list = list;
        cx.notify();
    }

    /// The playing media's bookmarks, by position.
    pub(super) fn bookmark_list(&self, cx: &App) -> &[Bookmark] {
        match &self.queue_panel.saved_bookmarks.loaded {
            Some(loaded) if self.player_media(cx).as_ref() == Some(&loaded.media) => &loaded.list,
            _ => &[],
        }
    }

    /// Whether the playing media takes bookmarks now.
    pub(super) fn bookmarkable(&self, cx: &App) -> Bookmarkable {
        let Some(player) = self.loaded_player(cx) else { return Bookmarkable::NothingLoaded };
        let player = player.read(cx);
        if player.is_seekable() == Some(false) || player.duration().is_none() {
            return Bookmarkable::CantSeek;
        }
        match self.queue_panel.saved_bookmarks.loaded.as_ref().map(|loaded| &loaded.identity) {
            Some(Identity::Known(_)) => Bookmarkable::Yes,
            Some(Identity::Reading) => Bookmarkable::Reading,
            Some(Identity::Unknown) | None => Bookmarkable::Unstorable,
        }
    }

    /// "+" and Cmd+B: bookmarks where playback is, or picks the bookmark already there, and
    /// puts the Bookmarks tab's cursor on it.
    pub(in crate::main_window) fn add_bookmark(&mut self, cx: &mut Context<Self>) {
        if self.bookmarkable(cx) != Bookmarkable::Yes {
            return;
        }
        let Some(player) = self.loaded_player(cx) else { return };
        let player = player.read(cx);
        let position = to_millisecond(player.position());
        let title = player.metadata().title.clone();
        let Some(loaded) = &mut self.queue_panel.saved_bookmarks.loaded else { return };
        let (index, message) = match slot(&loaded.list, position) {
            Ok(index) => {
                (index, tr!("Already bookmarked · {name}", name = loaded.list[index].name))
            }
            Err(index) => {
                let name = next_name(&media_title(title.as_deref(), &loaded.media), &loaded.list);
                let bookmark = Bookmark { position, name };
                loaded.list.insert(index, bookmark.clone());
                let message = tr!("Bookmark added · {name}", name = bookmark.name);
                self.store_bookmarks(move |c, identity| bookmarks::add(c, identity, &bookmark), cx);
                (index, message)
            }
        };
        self.queue_panel.point_at_bookmark(index);
        self.show_osd(message, cx);
        cx.notify();
    }

    /// Removes the bookmark at `index` of [`Self::bookmark_list`].
    pub(super) fn remove_bookmark(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(loaded) = &mut self.queue_panel.saved_bookmarks.loaded else { return };
        if index >= loaded.list.len() {
            return;
        }
        let position = loaded.list.remove(index).position;
        self.store_bookmarks(move |c, identity| bookmarks::remove(c, identity, position), cx);
        cx.notify();
    }

    /// Renames the bookmark at `index`; a blank name keeps the old one.
    pub(super) fn rename_bookmark(&mut self, index: usize, name: &str, cx: &mut Context<Self>) {
        let name = name.trim().to_owned();
        let Some(loaded) = &mut self.queue_panel.saved_bookmarks.loaded else { return };
        let Some(bookmark) = loaded.list.get_mut(index) else { return };
        if name.is_empty() || name == bookmark.name {
            return;
        }
        bookmark.name.clone_from(&name);
        let position = bookmark.position;
        self.store_bookmarks(
            move |c, identity| bookmarks::rename(c, identity, position, &name),
            cx,
        );
        cx.notify();
    }

    /// Goes to the bookmark at `index`, saying its name over the video.
    pub(super) fn go_to_bookmark(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(bookmark) = self.bookmark_list(cx).get(index).cloned() else { return };
        let Some(player) = self.seekable_player(cx) else { return };
        let duration = player.read(cx).duration();
        if past_the_end(bookmark.position, duration) {
            let name = &bookmark.name;
            self.show_osd(tr!("{name} is past the end of this media", name), cx);
            return;
        }
        self.seek_player(SeekTarget::Absolute(bookmark.position), cx);
        self.show_osd(bookmark.name, cx);
    }

    /// Writes a change to the playing media's bookmarks on the history thread.
    fn store_bookmarks(
        &self,
        write: impl FnOnce(&rusqlite::Connection, &str) -> rusqlite::Result<()> + Send + 'static,
        cx: &App,
    ) {
        let Some(Loaded { identity: Identity::Known(identity), .. }) =
            &self.queue_panel.saved_bookmarks.loaded
        else {
            return;
        };
        let Some(history) = History::get(cx) else { return };
        let identity = identity.clone();
        history.with_database(move |connection| {
            if let Err(error) = write(connection, &identity) {
                tracing::warn!("bookmarks: couldn't save the change: {error}");
            }
        });
    }
}

/// Whether a bookmark at `position` lies at or past the end of media `duration` long.
pub(super) fn past_the_end(position: Duration, duration: Option<Duration>) -> bool {
    duration.is_some_and(|duration| position >= duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bookmark_at_or_after_the_length_is_past_the_end() {
        let length = Some(Duration::from_secs(60));
        assert!(past_the_end(Duration::from_secs(60), length));
        assert!(!past_the_end(Duration::from_secs(59), length));
        assert!(!past_the_end(Duration::from_secs(90), None));
    }
}
