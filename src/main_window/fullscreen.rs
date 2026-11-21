//! Fullscreen (S1.4.2): F toggles it (a double-click on the video does too, in
//! [`VideoView`](crate::video::VideoView)), and Escape leaves it before Escape means anything
//! else. Escape leaves Hide UI mode (S2.8.2) first, so fullscreen with Hide UI always has a
//! keyboard way back: one Escape brings the controls, the next leaves fullscreen.
//!
//! The OS does the switch and keeps the windowed bounds: macOS uses native fullscreen (its own
//! Space, so Mission Control and Cmd+Tab behave as for any app), Windows and Linux a borderless
//! window on the window's current monitor. Leaving restores the bounds the window had, and the
//! saved geometry keeps those bounds rather than the screen's.

use std::time::Instant;

use gpui::{Action, Context, Div, Window, prelude::*};

use super::MainWindow;
use crate::keymap::actions::{ExitFullscreen, ToggleFullscreen};
use crate::keymap::commands;

impl MainWindow {
    /// Adds the fullscreen action handlers to `root`.
    pub(super) fn fullscreen_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, action: &ToggleFullscreen, window, _| {
            if this.admit(action) {
                window.toggle_fullscreen();
            }
        }))
        .on_action(cx.listener(Self::exit_fullscreen))
    }

    /// Escape: leaves Hide UI mode, else fullscreen, or, when windowed, lets the key mean what
    /// it means elsewhere (closing a menu or a dialog). Keymap actions run before key listeners,
    /// so in fullscreen Escape always leaves fullscreen first.
    fn exit_fullscreen(
        &mut self,
        action: &ExitFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.auto_hide.hide_ui {
            if self.admit(action) {
                self.set_hide_ui(false, cx);
            }
            return;
        }
        if !window.is_fullscreen() {
            cx.propagate();
            return;
        }
        // A held Escape must not re-enter fullscreen while the OS is still animating out of it.
        if self.admit(action) {
            window.toggle_fullscreen();
        }
    }

    /// Whether `action`'s key press is a fresh one rather than a held key's repeat.
    fn admit(&mut self, action: &dyn Action) -> bool {
        commands::find(action).is_none_or(|command| self.repeat.admit(command, Instant::now()))
    }
}
