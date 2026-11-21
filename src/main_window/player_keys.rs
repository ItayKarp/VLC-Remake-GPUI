//! The player's shortcuts in the main window (S0.5.2, S0.5.6). Every player
//! command passes the [`RepeatGate`] first, so a held Space toggles once and a held
//! arrow seeks at a steady rate. Fullscreen (S1.4.2, `fullscreen.rs`) and the volume
//! keys (S1.5.1) work; until the player exists (Stage 2) any other admitted command is
//! only logged, and S2.10.3 gives each one its effect.

use std::time::Instant;

use gpui::{Action, Context, Div, Window, prelude::*};

use super::MainWindow;
use crate::engine::values::VolumeChange;
use crate::keymap::actions as act;
use crate::keymap::commands;

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

    fn run_player_command(&mut self, action: &dyn Action, _: &mut Window, cx: &mut Context<Self>) {
        let Some(command) = commands::find(action) else {
            return;
        };
        if !self.repeat.admit(command, Instant::now()) {
            return;
        }
        if action.partial_eq(&act::VolumeUp) {
            self.step_volume(VolumeChange::Up, cx);
        } else if action.partial_eq(&act::VolumeDown) {
            self.step_volume(VolumeChange::Down, cx);
        } else {
            tracing::debug!("{}: no player yet (Stage 2)", command.label);
        }
    }
}
