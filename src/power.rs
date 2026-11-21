//! Keeps the display awake while video plays (S1.4.3).
//!
//! [`wants_awake`] says when: video playing in a window that shows. A pause, stop, minimized or
//! hidden window, or audio-only media lets the display sleep again. [`KeepAwake`] holds the OS
//! request for as long as it lives:
//! - macOS: an `IOPMAssertion` that prevents idle display sleep.
//! - Windows: `SetThreadExecutionState` on the UI thread.
//! - Linux: `org.freedesktop.ScreenSaver.Inhibit` over D-Bus, else a `systemd-inhibit` child.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use crate::engine::state::PlaybackState;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// Who asks, and why, where the OS shows it (Activity Monitor, `systemd-inhibit --list`).
const APP_NAME: &str = "VLC";
const REASON: &str = "Playing video";

/// What decides whether the display should stay awake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playback {
    /// The player's state.
    pub state: PlaybackState,
    /// Whether the media shows video (not audio only).
    pub has_video: bool,
    /// Whether the window shows (not minimized or fully covered).
    pub visible: bool,
}

/// Whether the display should stay awake: video playing, or buffering mid-play, where it shows.
pub fn wants_awake(playback: Playback) -> bool {
    let playing = matches!(playback.state, PlaybackState::Playing | PlaybackState::Buffering);
    playing && playback.has_video && playback.visible
}

/// Holds the display awake until dropped.
pub struct KeepAwake {
    _request: platform::Request,
}

impl KeepAwake {
    /// Asks the OS to keep the display awake. `None` if it refused; playback goes on anyway.
    pub fn acquire() -> Option<Self> {
        match platform::Request::new(APP_NAME, REASON) {
            Ok(request) => Some(Self { _request: request }),
            Err(error) => {
                tracing::warn!("power: couldn't keep the display awake: {error:#}");
                None
            }
        }
    }
}

/// Holds a [`KeepAwake`] exactly while [`wants_awake`] says so.
#[derive(Default)]
pub struct DisplaySleep {
    held: Option<KeepAwake>,
    /// Whether the last update wanted it, so a refused request isn't retried on every event.
    wanted: bool,
}

impl DisplaySleep {
    /// Acquires or releases the request to match `playback`.
    pub fn update(&mut self, playback: Playback) {
        let wanted = wants_awake(playback);
        if wanted == self.wanted {
            return;
        }
        self.wanted = wanted;
        self.held = if wanted { KeepAwake::acquire() } else { None };
        tracing::debug!("power: display sleep {}", if wanted { "blocked" } else { "allowed" });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playback(state: PlaybackState) -> Playback {
        Playback { state, has_video: true, visible: true }
    }

    #[test]
    fn playing_video_keeps_the_display_awake() {
        assert!(wants_awake(playback(PlaybackState::Playing)));
        assert!(wants_awake(playback(PlaybackState::Buffering)));
    }

    #[test]
    fn pause_stop_and_end_let_it_sleep() {
        for state in [
            PlaybackState::Idle,
            PlaybackState::Opening,
            PlaybackState::Paused,
            PlaybackState::Stopped,
            PlaybackState::Ended,
        ] {
            assert!(!wants_awake(playback(state)), "{state:?}");
        }
    }

    #[test]
    fn audio_only_or_a_hidden_window_lets_it_sleep() {
        let audio = Playback { has_video: false, ..playback(PlaybackState::Playing) };
        let hidden = Playback { visible: false, ..playback(PlaybackState::Playing) };
        assert!(!wants_awake(audio));
        assert!(!wants_awake(hidden));
    }
}
