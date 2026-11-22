//! External audio tracks (S4.8.1's Add Audio Tracks): audio files picked for a library file are
//! added to it as tracks and the first one plays. A file that isn't playing opens first, and the
//! files join it once its player has it. An added track rarely lines up with the video, so the
//! audio delay keys (J and K, as in VLC) move it earlier or later.

use std::path::PathBuf;

use gpui::{Context, PathPromptOptions, Window};

use super::MainWindow;
use crate::app::media::Media;
use crate::app::request::OpenRequest;
use crate::engine::values::Offset;
use crate::keymap::{self, actions as act};
use crate::opening::kind::{self, Kind};
use crate::ui::toast::{Level, Toast};

/// How far one press of J or K moves the audio.
const DELAY_STEP_MS: i64 = 50;

/// Which way the audio delay keys move the audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Shift {
    /// Plays it sooner (J).
    Earlier,
    /// Plays it later (K).
    Later,
}

/// Audio files waiting for their media to open.
#[derive(Default)]
pub(super) struct PendingAudio(Option<(Media, Vec<PathBuf>)>);

impl MainWindow {
    /// Moves the audio one step `shift`, and says where it is now.
    pub(super) fn shift_audio_delay(&mut self, shift: Shift, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let step = match shift {
            Shift::Earlier => -DELAY_STEP_MS,
            Shift::Later => DELAY_STEP_MS,
        };
        let delay = Offset::from_millis(player.read(cx).audio_delay().as_millis() + step);
        player.update(cx, |player, cx| player.set_audio_delay(delay, cx));
        self.show_osd(delay_message(delay), cx);
    }

    /// Asks for audio files and plays `media` with them added as tracks.
    pub(super) fn prompt_for_audio_tracks(
        media: Media,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            this.update_in(cx, |this, window, cx| this.add_audio_tracks(media, paths, window, cx))
                .ok();
        })
        .detach();
    }

    /// Adds the media files among `paths` to `media` as audio tracks: now if it is playing,
    /// else once it opens.
    fn add_audio_tracks(
        &mut self,
        media: Media,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (audio, others): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|path| kind::by_extension(path) == Some(Kind::Media));
        if let Some(other) = others.first() {
            let name = Media::File(other.clone()).display_name();
            self.show_toast(Toast::new(Level::Info, format!("{name} has no audio to add.")), cx);
        }
        if audio.is_empty() {
            return;
        }
        let playing = self.loaded_player(cx).and_then(|player| player.read(cx).media().cloned());
        self.audio_tracks.0 = Some((media.clone(), audio));
        if playing.as_ref() == Some(&media) {
            self.attach_pending_audio(cx);
        } else {
            self.open(OpenRequest::replace(vec![media]), window, cx);
        }
    }

    /// Adds the waiting audio files once the player has their media (a player event).
    pub(super) fn attach_pending_audio(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let PendingAudio(Some((media, _))) = &self.audio_tracks else { return };
        let player_state = player.read(cx);
        if player_state.media() != Some(media) || !player_state.state().is_active() {
            return;
        }
        let Some((_, files)) = self.audio_tracks.0.take() else { return };
        let added = player.update(cx, |player, cx| files.iter().all(|f| player.add_audio(f, cx)));
        let text = if added { added_message(&files, cx) } else { "Couldn't add the audio.".into() };
        let level = if added { Level::Success } else { Level::Error };
        self.show_toast(Toast::new(level, text), cx);
    }
}

/// What the toast says once `files` are added: which, and how to sync them.
fn added_message(files: &[PathBuf], cx: &gpui::App) -> String {
    let what = match files {
        [file] => {
            format!("Added “{}” as an audio track.", Media::File(file.clone()).display_name())
        }
        files => format!("Added {} audio tracks.", files.len()),
    };
    let earlier = keymap::shortcut(&act::AudioDelayEarlier, cx);
    let later = keymap::shortcut(&act::AudioDelayLater, cx);
    match (earlier, later) {
        (Some(earlier), Some(later)) => format!("{what} Out of sync? Press {earlier} or {later}."),
        _ => what,
    }
}

/// The OSD's text for an audio delay: "Audio delay: −150 ms".
fn delay_message(delay: Offset) -> String {
    match delay.as_millis() {
        0 => "Audio delay: 0 ms".to_owned(),
        millis if millis < 0 => format!("Audio delay: −{} ms", millis.unsigned_abs()),
        millis => format!("Audio delay: +{millis} ms"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delay_reads_with_its_sign() {
        assert_eq!(delay_message(Offset::from_millis(0)), "Audio delay: 0 ms");
        assert_eq!(delay_message(Offset::from_millis(-150)), "Audio delay: −150 ms");
        assert_eq!(delay_message(Offset::from_millis(50)), "Audio delay: +50 ms");
    }
}
