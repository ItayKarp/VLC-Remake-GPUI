//! The player's shortcuts in the main window (S0.5.2, S0.5.6). Every player
//! command passes the [`RepeatGate`] first, so a held Space toggles once and a held
//! arrow seeks at a steady rate. Fullscreen (S1.4.2, `fullscreen.rs`), the volume
//! keys (S1.5.1), track cycling (S1.6.2, B and V), the transport (S2.2: Space, S, P, N),
//! seeking (S2.3.4: the arrows with their modifiers, Home and End), the speed (S2.7.3: `[`,
//! `]` and `=`), the audio delay (S4.8.1: J and K), showing or hiding the interface (S2.8: I and Ctrl+\) and the video's menu
//! (S2.9: Shift+F10 and the context-menu key), frame stepping (`,` and `.`) and T (S2.10.3)
//! work, Q shows the queue (S3.2) and L the library sidebar (S4.5). A command whose story hasn't
//! shipped (X, Shift+S, Shift+R) says "Coming soon" over the video in debug builds and is only
//! logged in release builds.

use std::time::Instant;

use gpui::{Action, Context, Div, Window, prelude::*};

use super::MainWindow;
use super::audio_tracks::Shift;
use super::seeking::{Edge, Way};
use super::speed::rules::SpeedChange;
use crate::engine::tracks::TrackKind;
use crate::engine::values::{StepDirection, VolumeChange};
use crate::keymap::actions as act;
use crate::keymap::commands::{self, Command};
use crate::settings::SkipLength;

/// Registers a gated handler for each listed player action on `$element`.
macro_rules! player_actions {
    ($element:expr, $cx:expr, [$($action:ident),* $(,)?]) => {
        $element$(.on_action($cx.listener(|this, action: &act::$action, window, cx| {
            this.run_player_command(action, window, cx);
        })))*
    };
}

impl MainWindow {
    /// Adds the player's action handlers and the key-up and blur hooks that end a
    /// held key.
    pub(super) fn player_keys(root: Div, cx: &mut Context<Self>) -> Div {
        let root = root.on_key_up(cx.listener(|this, _, _, _| this.repeat.release_all()));
        let root = Self::fullscreen_actions(root, cx);
        let root = Self::playback_actions(root, cx);
        player_actions!(
            root,
            cx,
            [
                CycleSubtitleTrack,
                CycleAudioTrack,
                AudioDelayEarlier,
                AudioDelayLater,
                VolumeUp,
                VolumeDown,
                ShowTime,
                ShowControls,
                HideInterface,
                ToggleQueue,
                ToggleFileBrowser,
                Snapshot,
                Record,
                ShowContextMenu,
            ]
        )
    }

    /// The transport, play order, seeking, frame step and speed commands.
    fn playback_actions(root: Div, cx: &mut Context<Self>) -> Div {
        player_actions!(
            root,
            cx,
            [
                PlayPause,
                Stop,
                SeekBackward,
                SeekForward,
                SeekBackwardVeryShort,
                SeekForwardVeryShort,
                SeekBackwardMedium,
                SeekForwardMedium,
                SeekBackwardLong,
                SeekForwardLong,
                SeekToStart,
                SeekToEnd,
                PreviousItem,
                NextItem,
                CycleRepeat,
                ToggleShuffle,
                PreviousFrame,
                NextFrame,
                NextProgram,
                Faster,
                Slower,
                NormalSpeed,
            ]
        )
    }

    /// Forgets held keys when the window loses focus: their key-up goes elsewhere.
    pub(super) fn release_keys_on_blur(window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_activation(window, |this, window, _| {
            if !window.is_window_active() {
                this.repeat.release_all();
            }
        })
        .detach();
    }

    /// Runs `action` as its key does: through the held-key gate.
    pub(super) fn run_player_command(
        &mut self,
        action: &dyn Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(command) = commands::find(action) else {
            return;
        };
        if self.repeat.admit(command, Instant::now()) && !self.perform(action, window, cx) {
            self.not_yet(command, cx);
        }
    }

    /// A key whose command has no effect yet: says so over the video in debug builds.
    fn not_yet(&mut self, command: &Command, cx: &mut Context<Self>) {
        tracing::debug!("{}: not implemented yet", command.label);
        if cfg!(debug_assertions) {
            self.show_osd(coming_soon(command.label), cx);
        }
    }

    /// Runs the player command `action`, as its key does but without the held-key gate (a
    /// button's clicks each count). Returns `false` when nothing handles it yet.
    pub(super) fn perform(
        &mut self,
        action: &dyn Action,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, handler)) = HANDLERS.iter().find(|(handled, _)| handled.partial_eq(action))
        else {
            return false;
        };
        handler(self, window, cx);
        true
    }

    /// Whether [`Self::perform`] handles `action`.
    pub(super) fn handles(action: &dyn Action) -> bool {
        HANDLERS.iter().any(|(handled, _)| handled.partial_eq(action))
    }
}

/// Runs one player command on the main window.
type Handler = fn(&mut MainWindow, &mut Window, &mut Context<MainWindow>);

/// The player commands that do something, and what.
const HANDLERS: &[(&dyn Action, Handler)] = &[
    (&act::PlayPause, MainWindow::play_pause),
    (&act::Stop, MainWindow::stop),
    (&act::PreviousItem, MainWindow::previous),
    (&act::NextItem, MainWindow::next),
    (&act::CycleRepeat, MainWindow::cycle_repeat),
    (&act::ToggleShuffle, MainWindow::toggle_shuffle),
    (&act::SeekBackward, |this, _, cx| this.skip(Way::Back, SkipLength::Short, cx)),
    (&act::SeekForward, |this, _, cx| this.skip(Way::Forward, SkipLength::Short, cx)),
    (&act::SeekBackwardVeryShort, |this, _, cx| this.skip(Way::Back, SkipLength::VeryShort, cx)),
    (&act::SeekForwardVeryShort, |this, _, cx| this.skip(Way::Forward, SkipLength::VeryShort, cx)),
    (&act::SeekBackwardMedium, |this, _, cx| this.skip(Way::Back, SkipLength::Medium, cx)),
    (&act::SeekForwardMedium, |this, _, cx| this.skip(Way::Forward, SkipLength::Medium, cx)),
    (&act::SeekBackwardLong, |this, _, cx| this.skip(Way::Back, SkipLength::Long, cx)),
    (&act::SeekForwardLong, |this, _, cx| this.skip(Way::Forward, SkipLength::Long, cx)),
    (&act::SeekToStart, |this, _, cx| this.seek_to(Edge::Start, cx)),
    (&act::SeekToEnd, |this, _, cx| this.seek_to(Edge::End, cx)),
    (&act::PreviousFrame, |this, window, cx| this.step_frame(StepDirection::Backward, window, cx)),
    (&act::NextFrame, |this, window, cx| this.step_frame(StepDirection::Forward, window, cx)),
    (&act::ShowTime, MainWindow::show_time),
    (&act::VolumeUp, |this, window, cx| this.step_volume(VolumeChange::Up, window, cx)),
    (&act::VolumeDown, |this, window, cx| this.step_volume(VolumeChange::Down, window, cx)),
    (&act::CycleAudioTrack, |this, _, cx| this.cycle_track(TrackKind::Audio, cx)),
    (&act::CycleSubtitleTrack, |this, _, cx| this.cycle_track(TrackKind::Subtitle, cx)),
    (&act::AudioDelayEarlier, |this, _, cx| this.shift_audio_delay(Shift::Earlier, cx)),
    (&act::AudioDelayLater, |this, _, cx| this.shift_audio_delay(Shift::Later, cx)),
    (&act::Faster, |this, window, cx| this.change_speed(SpeedChange::Faster, window, cx)),
    (&act::Slower, |this, window, cx| this.change_speed(SpeedChange::Slower, window, cx)),
    (&act::NormalSpeed, |this, window, cx| this.change_speed(SpeedChange::Normal, window, cx)),
    (&act::ShowControls, MainWindow::summon_controls),
    (&act::HideInterface, MainWindow::toggle_hide_ui),
    (&act::ShowContextMenu, MainWindow::show_context_menu),
    (&act::ToggleQueue, MainWindow::toggle_queue_panel),
    (&act::ToggleFileBrowser, MainWindow::toggle_library_panel),
    // Its key has a handler of its own (`fullscreen.rs`); this one serves the menus.
    (&act::ToggleFullscreen, |_, window, _| window.toggle_fullscreen()),
];

/// What a key whose command has no effect yet says in debug builds.
fn coming_soon(label: &str) -> String {
    format!("Coming soon: {label}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::menus;

    /// Onboarding's third step teaches these keys (S2.10.3).
    const ONBOARDING_KEYS: &[&dyn Action] = &[
        &act::PlayPause,
        &act::SeekBackward,
        &act::SeekForward,
        &act::PreviousItem,
        &act::NextItem,
        &act::PreviousFrame,
        &act::NextFrame,
        &act::CycleSubtitleTrack,
        &act::CycleAudioTrack,
        &act::ToggleFullscreen,
        &act::VolumeUp,
        &act::VolumeDown,
        &act::ToggleQueue,
        &act::ToggleFileBrowser,
        &act::Snapshot,
        &act::Record,
    ];

    #[test]
    fn every_onboarding_key_works_or_waits_for_its_story() {
        for action in ONBOARDING_KEYS {
            let works = MainWindow::handles(*action);
            assert!(works != menus::is_pending(*action), "{action:?}: works {works}");
        }
    }

    #[test]
    fn unfinished_commands_say_so() {
        assert_eq!(coming_soon("Take a snapshot"), "Coming soon: Take a snapshot");
    }
}
