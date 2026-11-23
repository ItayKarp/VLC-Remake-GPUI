//! The playing file's row in the library, for the bar's Favorite and Thumbnail (S8.7.6). It is
//! looked up on the history thread as each media opens, after history has recorded the play,
//! so a file played for the first time has its row by then. Streams have none.

use gpui::{App, Context};

use super::MainWindow;
use crate::app::media::Media;
use crate::db::media::{self, MediaId};
use crate::history::History;
use crate::settings::stored_path;

/// The media last looked up and its row, once known.
#[derive(Default)]
pub(super) struct PlayingId {
    media: Option<Media>,
    id: Option<MediaId>,
}

impl MainWindow {
    /// Looks up the row of the media the player opens.
    pub(super) fn look_up_playing_id(&mut self, cx: &mut Context<Self>) {
        let media = self.loaded_player(cx).and_then(|player| player.read(cx).media().cloned());
        self.playing_id = PlayingId { media: media.clone(), id: None };
        let (Some(history), Some(path)) =
            (History::get(cx), media.as_ref().and_then(Media::as_path))
        else {
            return;
        };
        let location = stored_path::encode(path);
        let answer = history.query("player: the playing file's row", move |connection| {
            media::find_at(connection, &location).map(|row| row.map(|row| row.id))
        });
        cx.spawn(async move |this, cx| {
            let Ok(Some(Some(id))) = answer.await else { return };
            this.update(cx, |this, cx| this.take_playing_id(media.as_ref(), id, cx)).ok();
        })
        .detach();
    }

    /// Keeps `id` as the row of `media`, if it still plays.
    fn take_playing_id(&mut self, media: Option<&Media>, id: MediaId, cx: &mut Context<Self>) {
        if self.playing_id.media.as_ref() == media {
            self.playing_id.id = Some(id);
            cx.notify();
        }
    }

    /// The playing file's row in the library, once known.
    pub(super) fn playing_id(&self, cx: &App) -> Option<MediaId> {
        let player = self.loaded_player(cx)?;
        let playing = player.read(cx).media();
        (playing == self.playing_id.media.as_ref()).then_some(self.playing_id.id).flatten()
    }
}
