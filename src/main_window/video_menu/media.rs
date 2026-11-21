//! The video menu's Video, Audio and Subtitle submenus (S2.9.3).
//!
//! - **Video:** Video Track ▸ and Fullscreen. Aspect ratio, crop and zoom aren't in the engine
//!   yet, and Take Snapshot comes with S8.4, so they aren't listed.
//! - **Audio:** Audio Track ▸, Audio Device ▸ (as in the track popover, S2.6.3), Stereo Mode ▸
//!   (Settings → Audio "Speaker setup"), Mute, and the volume keys' commands. The visualizer
//!   comes with S7.8.
//! - **Subtitle:** Subtitle Track ▸ (added files by their names), Add Subtitle File…, and the
//!   subtitle delay in 50 ms steps, said over the video.
//!
//! A track menu lists "None" and each track of its kind with the playing one checked, like the
//! track popover; picking one counts as the user's pick (S2.6.2).

use gpui::{App, Context, Entity, WeakEntity, Window};

use super::{NOTHING_PLAYING, on_main, run};
use crate::assets::icons;
use crate::engine::audio::{OutputDevice, SpeakerSetup};
use crate::engine::player::Player;
use crate::engine::tracks::{SubtitleFile, TrackId, TrackKind};
use crate::engine::values::Offset;
use crate::keymap::actions as act;
use crate::main_window::MainWindow;
use crate::settings;
use crate::ui::menu::MenuItem;

/// How much one Increase or Decrease Delay moves the subtitles, as VLC's G and H keys do.
const DELAY_STEP_MS: i64 = 50;

impl MainWindow {
    /// Video ▸.
    pub(super) fn video_submenu(
        &self,
        main: &WeakEntity<Self>,
        window: &Window,
        cx: &App,
    ) -> MenuItem {
        let fullscreen = if window.is_fullscreen() { "Exit Fullscreen" } else { "Fullscreen" };
        let mut items: Vec<MenuItem> = self
            .loaded_player(cx)
            .map(|player| track_menu(TrackKind::Video, &player, cx))
            .into_iter()
            .collect();
        items.push(run(main, &act::ToggleFullscreen, fullscreen, cx).icon(icons::FULLSCREEN));
        MenuItem::submenu("Video", items).icon(icons::VIDEO)
    }

    /// Audio ▸.
    pub(super) fn audio_submenu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let Some(player) = self.loaded_player(cx) else {
            return MenuItem::submenu("Audio", Vec::new())
                .icon(icons::AUDIO)
                .why_disabled(NOTHING_PLAYING);
        };
        let muted = player.read(cx).is_muted();
        let mute = player.clone();
        let items = vec![
            track_menu(TrackKind::Audio, &player, cx),
            device_menu(&player, cx),
            stereo_mode_menu(cx),
            MenuItem::Separator,
            MenuItem::check("Mute", muted, move |_, cx| mute.update(cx, Player::toggle_mute)),
            run(main, &act::VolumeUp, "Increase Volume", cx),
            run(main, &act::VolumeDown, "Decrease Volume", cx),
        ];
        MenuItem::submenu("Audio", items).icon(icons::AUDIO)
    }

    /// Subtitle ▸.
    pub(super) fn subtitle_submenu(&self, main: &WeakEntity<Self>, cx: &App) -> MenuItem {
        let Some(player) = self.loaded_player(cx) else {
            return MenuItem::submenu("Subtitle", Vec::new())
                .icon(icons::SUBTITLES)
                .why_disabled(NOTHING_PLAYING);
        };
        let delay = player.read(cx).subtitle_delay().as_millis();
        let step = |label, change: i64| {
            on_main(main, label, move |main, _, cx| main.move_subtitle_delay(change, cx))
        };
        let items = vec![
            track_menu(TrackKind::Subtitle, &player, cx),
            on_main(main, "Add Subtitle File…", |_, window, cx| {
                MainWindow::prompt_for_subtitles(window, cx);
            }),
            MenuItem::Separator,
            step("Increase Delay", DELAY_STEP_MS),
            step("Decrease Delay", -DELAY_STEP_MS),
            step("Reset Delay", -delay).disabled(delay == 0).why_disabled("No delay"),
        ];
        MenuItem::submenu("Subtitle", items).icon(icons::SUBTITLES)
    }

    /// Moves the subtitle delay by `change` milliseconds and says where it is now.
    fn move_subtitle_delay(&mut self, change: i64, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let delay = player.read(cx).subtitle_delay().as_millis() + change;
        player.update(cx, |player, cx| player.set_subtitle_delay(Offset::from_millis(delay), cx));
        self.show_osd(delay_message(delay), cx);
    }
}

/// "Video Track ▸" and so on: "None" and each track of `kind`, the playing one checked.
fn track_menu(kind: TrackKind, player: &Entity<Player>, cx: &App) -> MenuItem {
    let (title, why) = match kind {
        TrackKind::Video => ("Video Track", "No video tracks"),
        TrackKind::Audio => ("Audio Track", "No audio tracks"),
        TrackKind::Subtitle => ("Subtitle Track", "No subtitles"),
    };
    let read = player.read(cx);
    let tracks = read.tracks();
    let selected = tracks.selected(kind);
    let file_name = |id: TrackId| {
        read.subtitle_files().iter().find(|file| file.track == Some(id)).map(SubtitleFile::name)
    };
    let choices: Vec<(Option<TrackId>, String)> = tracks
        .of(kind)
        .map(|track| {
            let label = file_name(track.id).or_else(|| tracks.label(track.id));
            (Some(track.id), label.unwrap_or_default())
        })
        .collect();
    let items = if choices.is_empty() {
        Vec::new()
    } else {
        std::iter::once((None, "None".to_owned()))
            .chain(choices)
            .map(|(id, label)| {
                let player = player.clone();
                MenuItem::radio(label, id == selected, move |_, cx| {
                    player.update(cx, |player, cx| player.select_track(kind, id, cx));
                })
            })
            .collect()
    };
    MenuItem::submenu(title, items).why_disabled(why)
}

/// Audio Device ▸: the system default and each device, the one sound goes to checked.
fn device_menu(player: &Entity<Player>, cx: &App) -> MenuItem {
    let read = player.read(cx);
    let active = read.active_audio_device().clone();
    let devices = read
        .audio_devices()
        .iter()
        .map(|device| (OutputDevice::Device(device.id.clone()), device.name.clone()));
    let items = std::iter::once((OutputDevice::SystemDefault, "System default".to_owned()))
        .chain(devices)
        .map(|(device, name)| {
            let checked = device == active;
            MenuItem::radio(name, checked, move |_, cx| {
                // The settings observer passes it to the player.
                settings::update(cx, |s| device.id().clone_into(&mut s.local.audio.output_device));
            })
        });
    MenuItem::submenu("Audio Device", items.collect())
}

/// Stereo Mode ▸: Settings → Audio "Speaker setup".
fn stereo_mode_menu(cx: &App) -> MenuItem {
    let setup = settings::get(cx).prefs.audio.speaker_setup;
    let choices = [(SpeakerSetup::Automatic, "Automatic"), (SpeakerSetup::Stereo, "Stereo")];
    let items = choices.into_iter().map(|(choice, label)| {
        MenuItem::radio(label, choice == setup, move |_, cx| {
            settings::update(cx, |s| s.prefs.audio.speaker_setup = choice);
        })
    });
    MenuItem::submenu("Stereo Mode", items.collect())
}

/// "Subtitle delay 150 ms", "Subtitle delay -50 ms".
fn delay_message(millis: i64) -> String {
    format!("Subtitle delay {millis} ms")
}

#[cfg(test)]
mod tests {
    use super::delay_message;

    #[test]
    fn delay_messages() {
        assert_eq!(delay_message(150), "Subtitle delay 150 ms");
        assert_eq!(delay_message(-50), "Subtitle delay -50 ms");
        assert_eq!(delay_message(0), "Subtitle delay 0 ms");
    }
}
