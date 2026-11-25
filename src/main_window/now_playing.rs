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
use super::{device_failure, subtitle_style};
use crate::app::media::Media;
use crate::engine;
use crate::engine::backend::{Backend, EventSender};
use crate::engine::player::{Player, PlayerEvent};
use crate::engine::state::{ErrorKind, PlaybackState};
use crate::library;
use crate::power::Playback;
use crate::settings::{self, SettingsStore};
use crate::theme::Palette;
use crate::theme::text_px;
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
            let text = tr!("Open a video first, then add subtitles.");
            self.show_toast(Toast::new(Level::Info, text), cx);
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
        self.attach_player(output, backend, window, cx)
    }

    /// Makes the player on the backend `make_backend` builds, its pictures shown from `output`,
    /// and wires it to the window: its events, the video area, audio, tracks and picture.
    pub(super) fn attach_player(
        &mut self,
        output: Output,
        make_backend: impl FnOnce(EventSender) -> Box<dyn Backend>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Player> {
        let player = cx.new(|cx| Player::new(make_backend, cx));
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
        let here = !self.windowing.separate.is_separate() && !self.windowing.pip.is_shown();
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

    /// Brings back what is remembered about the media the player opens.
    fn opening(&mut self, cx: &mut Context<Self>) {
        self.windowing.start_fullscreen = true;
        self.restore_subtitle_syncs(cx);
        self.restore_audio_delays(cx);
        self.load_bookmarks(cx);
        self.load_linked_subtitles(cx);
    }

    fn on_player_event(
        &mut self,
        player: &Entity<Player>,
        event: &PlayerEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.on_side_events(event, cx);
        match event {
            PlayerEvent::EndReached => self.item_ended(window, cx),
            PlayerEvent::Error(kind) => self.failed(*kind, window, cx),
            PlayerEvent::StateChanged(state) => {
                if *state == PlaybackState::Playing {
                    self.skips.played();
                    self.start_compact(window, cx);
                }
                if *state == PlaybackState::Opening {
                    self.opening(cx);
                }
                self.found_playing(*state, cx);
                self.notify_now_playing(*state, cx);
                self.announce_state(*state, cx);
                self.attach_pending_audio(cx);
                let busy = matches!(
                    state,
                    PlaybackState::Opening | PlaybackState::Buffering | PlaybackState::Playing
                );
                library::thumbnails::set_playing(player.entity_id(), busy, cx);
                self.reset_seek_bar(*state);
                self.update_display_sleep(window, cx);
                self.sync_video_window(window, cx);
                self.sync_pip(window, cx);
                cx.notify();
            }
            PlayerEvent::VideoSizeChanged(size) => {
                self.refit_pip(cx);
                self.update_display_sleep(window, cx);
                self.sync_video_window(window, cx);
                if let Some(size) = size {
                    self.fit_to_video((size.width as f32, size.height as f32), window, cx);
                    self.start_fullscreen(window, cx);
                }
            }
            PlayerEvent::HdrChanged(status) => self.warn_hdr(*status, cx),
            _ => self.on_audio_event(event, cx),
        }
        // The control bar shows the position, length, volume and what Play/Pause would do, and
        // the title bar names the media by its title tag (S2.1.2).
        cx.notify();
    }

    /// What the history, scrobbling, podcasts, recording, disc menus, casting and streaming
    /// follow of `event`.
    fn on_side_events(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        self.on_history_event(event, cx);
        self.on_scrobble_event(event, cx);
        if *event == PlayerEvent::EndReached {
            self.episode_played(cx);
        }
        self.on_recording_event(event, cx);
        self.on_disc_event(event, cx);
        self.on_cast_event(event, cx);
        self.on_stream_event(event, cx);
    }

    /// Playback failed: says why for a disc or device, and moves on in the queue.
    fn failed(&mut self, kind: ErrorKind, window: &mut Window, cx: &mut Context<Self>) {
        self.explain_device_failure(kind, cx);
        self.skip_failed(window, cx);
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
            .text_size(text_px(14.))
            .text_color(p.text)
            .child(text);
        let line = if cancellable {
            line.child(
                Button::new("cancel-open", tr!("Cancel"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.cancel(cx))),
            )
        } else {
            line
        };
        Some(line.into_any_element())
    }

    /// What to say, and whether Cancel applies.
    pub(super) fn status(&self, cx: &Context<Self>) -> Option<(SharedString, bool)> {
        if let Some(job) = self.jobs.running.last() {
            return Some((tr!("Opening {name}…", name = job.label).into(), true));
        }
        let (player, _) = self.player.as_ref()?;
        let player = player.read(cx);
        let name = player.media().map_or_default(Media::display_name);
        let text = match player.state() {
            PlaybackState::Idle => return None,
            PlaybackState::Opening => return Some((tr!("Opening {name}…", name).into(), true)),
            PlaybackState::Buffering => tr!("Buffering {name}…", name),
            PlaybackState::Playing => tr!("Playing {name}", name),
            PlaybackState::Paused => tr!("Paused: {name}", name),
            PlaybackState::Ended => tr!("Finished {name}", name),
            PlaybackState::Stopped => tr!("Stopped {name}", name),
            PlaybackState::Error(kind) => {
                let device = player.media().and_then(Media::as_device);
                let reason =
                    device.map_or(error_message(kind), |d| device_failure::reason(d, kind));
                tr!("Couldn't play {name}: {reason}", name, reason)
            }
        };
        Some((text.into(), false))
    }
}

/// Why something couldn't play, for the user.
fn error_message(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => tr!("the file doesn't exist."),
        ErrorKind::EmptyFile => tr!("the file is empty."),
        ErrorKind::NotAFile => tr!("it isn't a media file."),
        ErrorKind::Unreadable => tr!("the file can't be read."),
        ErrorKind::Locked => tr!("another program is using the file."),
        ErrorKind::OpenFailed => tr!("VLC can't open this media."),
        ErrorKind::NoPlayableStreams => tr!("it has nothing VLC can play."),
        ErrorKind::Corrupt => tr!("the file is damaged."),
        ErrorKind::Protected => tr!("protected content can't be played."),
        ErrorKind::BackendUnavailable => tr!("VLC's playback engine couldn't be loaded."),
        ErrorKind::SourceLost => tr!("the file or its drive went away."),
    }
}

#[cfg(test)]
pub(super) mod tests;
