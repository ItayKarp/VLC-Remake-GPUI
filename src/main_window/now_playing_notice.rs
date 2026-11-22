//! The now-playing notification (S5.4.3): when an item starts playing, a system notification
//! names it (its title tag, else its file name) and its artist or album, following Settings →
//! Interface "Show what starts playing": never, only while no VLC window is active (the
//! default), or always. It names each item once, however often it pauses and plays, and a newer
//! one replaces it. Clicking it brings VLC forward (`app::init`).

use gpui::{App, Context, SharedString, SystemNotification};

use super::MainWindow;
use super::title_bar::text::{Loaded, window_title};
use crate::app::media::Media;
use crate::engine::state::PlaybackState;
use crate::settings::{self, NowPlayingNotification};

/// The notification's tag, so each one replaces the last.
pub const NOW_PLAYING_TAG: &str = "now-playing";

/// The item last named, so it isn't named again when it resumes.
#[derive(Default)]
pub struct NowPlayingNotice {
    named: Option<Media>,
}

impl MainWindow {
    /// Names what started playing, if the setting asks for it now.
    pub(super) fn notify_now_playing(&mut self, state: PlaybackState, cx: &mut Context<Self>) {
        if state != PlaybackState::Playing {
            return;
        }
        let Some(notification) = self.now_playing_notification(cx) else { return };
        if wanted(cx) {
            cx.show_system_notification(notification);
        }
    }

    /// The notification for a newly playing item; `None` for the item already named.
    fn now_playing_notification(&mut self, cx: &App) -> Option<SystemNotification> {
        let (player, _) = self.player.as_ref()?;
        let player = player.read(cx);
        let media = player.media()?;
        if self.windowing.notice.named.as_ref() == Some(media) {
            return None;
        }
        self.windowing.notice.named = Some(media.clone());
        let metadata = player.metadata();
        let title = window_title(Some(Loaded { media, tag: metadata.title.as_deref() }), true);
        let body = metadata.artist.as_ref().or(metadata.album.as_ref()).cloned();
        Some(SystemNotification {
            tag: NOW_PLAYING_TAG.into(),
            title: title.into(),
            body: SharedString::from(body.unwrap_or_default()),
            actions: Vec::new(),
        })
    }
}

/// Whether the setting wants a notification now.
fn wanted(cx: &App) -> bool {
    match settings::get(cx).prefs.interface.now_playing_notification {
        NowPlayingNotification::Never => false,
        NowPlayingNotification::InBackground => cx.active_window().is_none(),
        NowPlayingNotification::Always => true,
    }
}
