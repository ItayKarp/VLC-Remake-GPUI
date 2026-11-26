//! The main window's side of the system tray (S8.16.1): the tray menu's Play/Pause, Previous and
//! Next run through the window's own transport, and with "Keep playing in the tray when the
//! window closes" closing the window hides it instead, playback going on, until Show VLC brings
//! it back. Where the OS can't hide a window (Linux), it is minimized instead.

use gpui::{Context, Window};

use super::MainWindow;
use crate::app::system_tray;
use crate::native_window;
use crate::tray::TrayCommand;

/// Whether the main window is hidden into the tray.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InTray {
    /// It shows, or is minimized like any window.
    #[default]
    No,
    /// Closing it hid it; Show VLC brings it back.
    Hidden,
}

impl MainWindow {
    /// Does what the tray menu asked, as the window's own controls would. Quit is the app's.
    pub fn run_tray_command(
        &mut self,
        command: TrayCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            TrayCommand::PlayPause => self.play_pause(window, cx),
            TrayCommand::Previous => self.previous(window, cx),
            TrayCommand::Next => self.next(window, cx),
            TrayCommand::ShowVlc => self.show_from_tray(window, cx),
            TrayCommand::Quit => {}
        }
    }

    /// Shows the window again if the tray hid it, and brings it forward.
    pub fn show_from_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.windowing.tray) == InTray::Hidden {
            native_window::set_hidden(window, false);
        }
        window.activate_window();
        cx.activate(true);
    }

    /// Whether the window was hidden into the tray.
    pub fn is_hidden_in_tray(&self) -> bool {
        self.windowing.tray == InTray::Hidden
    }

    /// Hides the window into the tray instead of closing it, if Settings asks for that. Returns
    /// whether it did, so the close is called off.
    pub fn hide_to_tray(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !system_tray::keeps_playing_when_closed(cx) {
            return false;
        }
        if native_window::set_hidden(window, true) {
            self.windowing.tray = InTray::Hidden;
        } else {
            window.minimize_window();
        }
        true
    }
}

#[cfg(test)]
mod tests;
