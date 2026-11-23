//! The engine's [`Player`] in the main window: created on first use, its events routed to
//! the queue (`queue_play.rs`) and the controls, and a status line that says what's happening
//! ("Opening …" with Cancel, or why something couldn't play).
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
use super::subtitle_style;
use crate::app::media::Media;
use crate::engine;
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::library;
use crate::power::Playback;
use crate::settings::{self, SettingsStore};
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::toast::{Level, Toast};
use crate::video::{Output, VideoView};

impl MainWindow {
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

    pub(super) fn is_playing(&self, cx: &Context<Self>) -> bool {
        self.player.as_ref().is_some_and(|(player, _)| player.read(cx).state().is_active())
    }

    /// The player, created on first use.
    pub(super) fn player(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Entity<Player> {
        if let Some((player, _)) = &self.player {
            return player.clone();
        }
        let output = Output::new();
        let subtitles = subtitle_style::style(&settings::get(cx).prefs.subtitles);
        let backend = engine::default_backend(output.sink(), Self::audio_config(cx), subtitles);
        let player = cx.new(|cx| Player::new(backend, cx));
        Self::start_at_default_speed(&player, cx);
        let events = cx.subscribe_in(&player, window, Self::on_player_event);
        // A player closed while playing mustn't hold the thumbnailer back (S4.4).
        let id = player.entity_id();
        cx.observe_release(&player, move |_, _, cx| {
            library::thumbnails::set_playing(id, false, cx);
        })
        .detach();
        self.video = Some(cx.new(|cx| VideoView::new(output, player.clone(), window, cx)));
        self.player = Some((player.clone(), events));
        self.set_up_audio(&player, cx);
        self.set_up_tracks(&player, cx);
        self.set_up_picture(&player, cx);
        player
    }

    /// The video area, while the player has media loaded and its video plays in this window.
    pub(super) fn showing_video(&self, cx: &App) -> Option<Entity<VideoView>> {
        let (player, _) = self.player.as_ref()?;
        let here = !self.windowing.separate.is_separate();
        (here && player.read(cx).state().is_active()).then(|| self.video.clone()).flatten()
    }

    /// Re-checks whether the display should stay awake when the window shows or hides, and when
    /// Settings → Video's "Stop the screensaver during playback" changes.
    pub(super) fn follow_visibility(window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        let visibility = cx.observe_window_visibility(window, |this, _, window, cx| {
            this.update_display_sleep(window, cx);
        });
        let setting = cx.observe_global_in::<SettingsStore>(window, |this, window, cx| {
            this.update_display_sleep(window, cx);
        });
        Subscription::join(visibility, setting)
    }

    /// Keeps the display awake while video plays, unless Settings → Video says not to.
    fn update_display_sleep(&mut self, window: &Window, cx: &App) {
        let Some((player, _)) = &self.player else { return };
        let player = player.read(cx);
        let wanted = settings::get(cx).prefs.video.keep_display_awake;
        self.display_sleep.update(Playback {
            state: player.state(),
            has_video: wanted && player.video_size().is_some(),
            visible: window.is_visible(),
        });
    }

    fn on_player_event(
        &mut self,
        player: &Entity<Player>,
        event: &PlayerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_history_event(event, cx);
        match event {
            PlayerEvent::EndReached => self.advance(window, cx),
            PlayerEvent::Error(_) => self.skip_failed(window, cx),
            PlayerEvent::StateChanged(state) => {
                if *state == PlaybackState::Playing {
                    self.skips.played();
                }
                if *state == PlaybackState::Opening {
                    self.windowing.start_fullscreen = true;
                    self.restore_subtitle_syncs(cx);
                    self.restore_audio_delays(cx);
                    self.load_linked_subtitles(cx);
                }
                self.found_playing(*state, cx);
                self.notify_now_playing(*state, cx);
                self.attach_pending_audio(cx);
                let busy = matches!(
                    state,
                    PlaybackState::Opening | PlaybackState::Buffering | PlaybackState::Playing
                );
                library::thumbnails::set_playing(player.entity_id(), busy, cx);
                self.reset_seek_bar(*state);
                self.update_display_sleep(window, cx);
                self.sync_video_window(window, cx);
                cx.notify();
            }
            PlayerEvent::VideoSizeChanged(size) => {
                self.update_display_sleep(window, cx);
                self.sync_video_window(window, cx);
                if let Some(size) = size {
                    self.fit_to_video((size.width as f32, size.height as f32), window, cx);
                    self.start_fullscreen(window, cx);
                }
            }
            PlayerEvent::HdrChanged(status) => self.warn_hdr(*status, cx),
            // The title bar names the media by its title tag (S2.1.2).
            PlayerEvent::MetadataChanged => cx.notify(),
            _ => self.on_audio_event(event, cx),
        }
        // The control bar shows the position, length, volume and what Play/Pause would do.
        cx.notify();
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
        if let Some(job) = self.jobs.running.last() {
            return Some((format!("Opening {}…", job.label).into(), true));
        }
        let (player, _) = self.player.as_ref()?;
        let player = player.read(cx);
        let name = player.media().map_or_default(Media::display_name);
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
