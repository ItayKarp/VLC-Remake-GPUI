//! What the main window plays until the player screen (Stage 2) and the queue (Stage 3)
//! exist: the opened list, played in order through the engine's [`Player`], and a status line
//! that says what's happening ("Opening …" with Cancel, or why something couldn't play).
//!
//! The player is created on the first open, so a window that opens nothing never loads
//! libVLC. Its video shows in a [`VideoView`] that replaces the empty state while media is
//! loaded, and the display stays awake while that video plays (S1.4). Its audio follows the
//! settings (`audio.rs`, S1.5).

use std::path::PathBuf;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, SharedString, Subscription, Window, div,
    prelude::*, px,
};

use super::MainWindow;
use crate::app::media::Media;
use crate::app::request::Placement;
use crate::engine;
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::power::Playback;
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::toast::{Level, Toast};
use crate::video::{Output, VideoView};

impl MainWindow {
    /// Puts `media` in the list, replacing it or adding to it, and plays the first new item
    /// unless something from the list is already playing.
    pub(super) fn play_list(
        &mut self,
        media: Vec<Media>,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let first_new = match placement {
            Placement::Replace => {
                self.files.clear();
                0
            }
            Placement::Enqueue => self.files.len(),
        };
        self.files.extend(media);
        if placement == Placement::Replace || !self.is_playing(cx) {
            self.play(first_new, window, cx);
        }
    }

    /// Attaches dropped subtitle files to what's playing, or explains why it can't.
    pub(super) fn attach_subtitles(&mut self, subtitles: &[PathBuf], cx: &mut Context<Self>) {
        if subtitles.is_empty() {
            return;
        }
        let player = self.player.as_ref().map(|(player, _)| player.clone());
        let attached = player.is_some_and(|player| {
            player
                .update(cx, |player, cx| subtitles.iter().all(|path| player.add_subtitle(path, cx)))
        });
        if !attached {
            self.show_toast(Toast::new(Level::Info, "Open a video first, then add subtitles."), cx);
        }
    }

    /// Plays item `index` of the list.
    fn play(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(media) = self.files.get(index).cloned() else {
            return;
        };
        self.current = Some(index);
        window.set_window_title(&media.display_name());
        let player = self.player(window, cx);
        player.update(cx, |player, cx| player.open(media, cx));
        // The video replaces the empty state and its focused Open button; with nothing focused
        // the player's keys would reach nothing.
        if self.open_button.is_focused(window) || window.focused(cx).is_none() {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    fn is_playing(&self, cx: &Context<Self>) -> bool {
        self.player.as_ref().is_some_and(|(player, _)| player.read(cx).state().is_active())
    }

    /// The player, created on first use.
    fn player(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Player> {
        if let Some((player, _)) = &self.player {
            return player.clone();
        }
        let output = Output::new();
        let backend = engine::default_backend(output.sink(), Self::audio_config(cx));
        let player = cx.new(|cx| Player::new(backend, cx));
        let events = cx.subscribe_in(&player, window, Self::on_player_event);
        self.video = Some(cx.new(|cx| VideoView::new(output, player.clone(), window, cx)));
        self.player = Some((player.clone(), events));
        self.set_up_audio(&player, cx);
        self.set_up_tracks(&player, cx);
        player
    }

    /// The video area, while the player has media loaded.
    pub(super) fn showing_video(&self, cx: &App) -> Option<Entity<VideoView>> {
        let (player, _) = self.player.as_ref()?;
        player.read(cx).state().is_active().then(|| self.video.clone()).flatten()
    }

    /// Re-checks whether the display should stay awake when the window shows or hides.
    pub(super) fn follow_visibility(window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.observe_window_visibility(window, |this, _, window, cx| {
            this.update_display_sleep(window, cx);
        })
    }

    fn update_display_sleep(&mut self, window: &Window, cx: &App) {
        let Some((player, _)) = &self.player else { return };
        let player = player.read(cx);
        self.display_sleep.update(Playback {
            state: player.state(),
            has_video: player.video_size().is_some(),
            visible: window.is_visible(),
        });
    }

    fn on_player_event(
        &mut self,
        _: &Entity<Player>,
        event: &PlayerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            // Until the queue (Stage 3) has its policies, the list plays straight through.
            PlayerEvent::EndReached => {
                if let Some(next) = self.current.map(|i| i + 1) {
                    self.play(next, window, cx);
                }
            }
            PlayerEvent::StateChanged(_) => {
                self.update_display_sleep(window, cx);
                cx.notify();
            }
            PlayerEvent::VideoSizeChanged(_) => self.update_display_sleep(window, cx),
            _ => self.on_audio_event(event, cx),
        }
    }

    /// Stops what is opening: resolving items, and the engine's open.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.cancel_opening(cx);
        if let Some((player, _)) = &self.player {
            player.update(cx, Player::stop);
        }
    }

    /// The status line under the drop zone, if there's anything to say.
    pub(super) fn render_status(&self, p: &Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (text, cancellable) = self.status(cx)?;
        let line = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .text_size(px(14.))
            .text_color(p.text)
            .child(text);
        let line = if cancellable {
            line.child(
                Button::new("cancel-open", "Cancel")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
            )
        } else {
            line
        };
        Some(line.into_any_element())
    }

    /// What to say, and whether Cancel applies.
    fn status(&self, cx: &Context<Self>) -> Option<(SharedString, bool)> {
        if let Some(job) = self.jobs.last() {
            return Some((format!("Opening {}…", job.label).into(), true));
        }
        let (player, _) = self.player.as_ref()?;
        let player = player.read(cx);
        let name = player.media().map(Media::display_name).unwrap_or_default();
        let text = match player.state() {
            PlaybackState::Idle => return None,
            PlaybackState::Opening => return Some((format!("Opening {name}…").into(), true)),
            PlaybackState::Buffering => format!("Buffering {name}…"),
            PlaybackState::Playing => format!("Playing {name}"),
            PlaybackState::Paused => format!("Paused: {name}"),
            PlaybackState::Ended => format!("Finished {name}"),
            PlaybackState::Stopped => format!("Stopped {name}"),
            PlaybackState::Error(kind) => format!("Couldn't play {name}: {}", error_message(kind)),
        };
        Some((text.into(), false))
    }
}

/// Why something couldn't play, for the user.
fn error_message(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "the file doesn't exist.",
        ErrorKind::EmptyFile => "the file is empty.",
        ErrorKind::NotAFile => "it isn't a media file.",
        ErrorKind::Unreadable => "the file can't be read.",
        ErrorKind::Locked => "another program is using the file.",
        ErrorKind::OpenFailed => "VLC can't open this media.",
        ErrorKind::NoPlayableStreams => "it has nothing VLC can play.",
        ErrorKind::Corrupt => "the file is damaged.",
        ErrorKind::Protected => "protected content can't be played.",
        ErrorKind::BackendUnavailable => "VLC's playback engine couldn't be loaded.",
        ErrorKind::SourceLost => "the file or its drive went away.",
    }
}
