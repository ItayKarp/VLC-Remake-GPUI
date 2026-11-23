//! The main window's side of casting (S8.11): it gives the player Settings → Recording's
//! Casting rows, plays on the device picked in the settings panel's Render row, saying where
//! media plays now over the video, and says so when the device media played on goes away.

use gpui::Context;

use super::MainWindow;
use crate::engine::cast::{CastDeviceId, CastSetup};
use crate::engine::player::PlayerEvent;
use crate::settings::Settings;
use crate::ui::toast::{Level, Toast};

/// What the message over the video says once media plays here again.
const PLAYING_HERE: &str = "Playing on this computer";

/// The casting setup in `settings`: "Discover cast devices" and "Cast quality".
pub(super) fn setup(settings: &Settings) -> CastSetup {
    let recording = &settings.prefs.recording;
    CastSetup { discover: recording.discover_cast_devices, quality: recording.cast_quality }
}

impl MainWindow {
    /// Plays on `device`, or here with `None`, and says where over the video.
    pub(super) fn cast_to(&mut self, device: Option<CastDeviceId>, cx: &mut Context<Self>) {
        let Some((player, _)) = &self.player else { return };
        let name = player.update(cx, |player, cx| {
            player.cast_to(device, cx);
            player.cast_target().map(|device| device.name.clone())
        });
        if device.is_some() && name.is_none() {
            return;
        }
        let message =
            name.map_or_else(|| PLAYING_HERE.to_owned(), |name| format!("Playing on {name}"));
        self.show_osd(message, cx);
    }

    /// Says when the device media played on went away.
    pub(super) fn on_cast_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        match event {
            PlayerEvent::CastLost(device) => {
                let text = format!("{} can't be found any more. Playing here again.", device.name);
                self.show_toast(Toast::new(Level::Info, text), cx);
            }
            PlayerEvent::CastDevicesChanged | PlayerEvent::CastChanged => cx.notify(),
            _ => {}
        }
    }
}
