//! The player's shortcuts in the main window (S0.5.2, S0.5.6). Every player
//! command passes the [`RepeatGate`] first, so a held Space toggles once and a held
//! arrow seeks at a steady rate. Fullscreen (S1.4.2, `fullscreen.rs`), the volume
//! keys (S1.5.1), track cycling (S1.6.2, B and V), the transport (S2.2: Space, S, P, N)
//! and seeking (S2.3.4: the arrows with their modifiers, Home and End) work; any other
//! admitted command is only logged until S2.10.3 gives it its effect.

use std::time::Instant;

use gpui::{Action, Context, Div, Window, prelude::*};

use super::MainWindow;
use super::seeking::{Edge, Way};
use crate::engine::tracks::TrackKind;
use crate::engine::values::VolumeChange;
use crate::keymap::actions as act;
use crate::keymap::commands;
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
                PreviousFrame,
                NextFrame,
                NextProgram,
                CycleSubtitleTrack,
                CycleAudioTrack,
                VolumeUp,
                VolumeDown,
                ShowTime,
                ShowControls,
                HideInterface,
                ToggleQueue,
                ToggleFileBrowser,
                Snapshot,
                Record,
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
            tracing::debug!("{}: no player yet (Stage 2)", command.label);
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
    (&act::VolumeUp, |this, window, cx| this.step_volume(VolumeChange::Up, window, cx)),
    (&act::VolumeDown, |this, window, cx| this.step_volume(VolumeChange::Down, window, cx)),
    (&act::CycleAudioTrack, |this, _, cx| this.cycle_track(TrackKind::Audio, cx)),
    (&act::CycleSubtitleTrack, |this, _, cx| this.cycle_track(TrackKind::Subtitle, cx)),
];
