//! The video menu's Video, Audio and Subtitle submenus (S2.9.3).
//!
//! - **Video:** Video Track ▸, the framing (Zoom ▸, Aspect Ratio ▸, Crop ▸, Flip ▸ and Rotate ▸,
//!   S7.2), Deinterlace ▸ and its Deinterlacing Method ▸ (S7.2.6), and Fullscreen. Like the
//!   player's settings panel, they change what plays for the session, not Settings (D24). Take
//!   Snapshot comes with S8.4.
//! - **Audio:** Audio Track ▸, Audio Device ▸ (as in the track popover, S2.6.3), Stereo Mode ▸
//!   (Settings → Audio "Speaker setup"), Visualizer ▸ (Settings → Audio "Audio visualizer",
//!   S7.8.1), Mute, and the volume keys' commands.
//! - **Subtitle:** Subtitle Track ▸ (files by their names), Add Subtitle File…, Text Encoding ▸
//!   for the file showing (S6.1.6), and the subtitle delay in 50 ms steps, said over the video.
//!
//! A track menu lists "None" and each track of its kind with the playing one checked, like the
//! track popover; picking one counts as the user's pick (S2.6.2).

use encoding_rs::Encoding;
use gpui::{App, Entity, WeakEntity, Window};

use super::{NOTHING_PLAYING, on_main, run};
use crate::assets::icons;
use crate::effects;
use crate::engine::audio::visualizer::Visualizer;
use crate::engine::audio::{OutputDevice, SpeakerSetup};
use crate::engine::player::Player;
use crate::engine::sidecar;
use crate::engine::tracks::{SubtitleFile, TrackId, TrackKind};
use crate::engine::values::Offset;
use crate::keymap::actions as act;
use crate::main_window::MainWindow;
use crate::settings;
use crate::settings::registry::{self, Control, TEXT_ENCODINGS, Value};
use crate::ui::menu::MenuItem;

/// The Video menu's submenus of a setting held for the session: its key and the title.
const SESSION_MENUS: [(&str, &str); 7] = [
    ("video.zoom", "Zoom"),
    ("video.aspect_ratio", "Aspect Ratio"),
    ("video.crop", "Crop"),
    ("video.flip", "Flip"),
    ("video.rotation", "Rotate"),
    ("video.deinterlace", "Deinterlace"),
    ("video.deinterlace_method", "Deinterlacing Method"),
];

impl MainWindow {
    /// Video ▸.
    pub(super) fn video_submenu(
        &self,
        main: &WeakEntity<Self>,
        window: &Window,
        cx: &App,
    ) -> MenuItem {
        let fullscreen =
            if self.video_is_fullscreen(window) { "Exit Fullscreen" } else { "Fullscreen" };
        let mut items: Vec<MenuItem> = self
            .loaded_player(cx)
            .map(|player| track_menu(TrackKind::Video, &player, cx))
            .into_iter()
            .collect();
        items.push(MenuItem::Separator);
        items.extend(SESSION_MENUS.iter().map(|(key, title)| session_menu(key, title, cx)));
        items.push(MenuItem::Separator);
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
            visualizer_menu(cx),
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
        let sync = player.read(cx).subtitle_sync();
        let showing = player.read(cx).subtitle_key().is_some();
        let reset = on_main(main, "Reset Delay", move |main, _, cx| {
            main.set_subtitle_sync(sync.with_delay(Offset::default()), cx);
        });
        let why = if showing { "No delay" } else { "No subtitles showing" };
        let items = vec![
            track_menu(TrackKind::Subtitle, &player, cx),
            run(main, &act::AddSubtitleFile, "Add Subtitle File…", cx),
            encoding_menu(&player, cx),
            MenuItem::Separator,
            run(main, &act::SubtitleDelayLater, "Increase Delay", cx),
            run(main, &act::SubtitleDelayEarlier, "Decrease Delay", cx),
            reset.disabled(sync.delay() == Offset::default()).why_disabled(why),
        ];
        MenuItem::submenu("Subtitle", items).icon(icons::SUBTITLES)
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

/// Text Encoding ▸ (S6.1.6): what the subtitle file showing is read in. Automatic names the
/// encoding it found; picking one reads the file again in it.
fn encoding_menu(player: &Entity<Player>, cx: &App) -> MenuItem {
    let read = player.read(cx);
    let shown = read.tracks().subtitle;
    let file = read.subtitle_files().iter().find(|file| shown.is_some() && file.track == shown);
    let Some((file, track)) = file.filter(|file| sidecar::is_text(&file.path)).zip(shown) else {
        return MenuItem::submenu("Text Encoding", Vec::new())
            .why_disabled("The subtitles showing aren't from a text file");
    };
    let reread = |encoding: Option<&'static Encoding>| {
        let player = player.clone();
        move |_: &mut Window, cx: &mut App| {
            player.update(cx, |player, cx| player.reread_subtitle(track, encoding, cx));
        }
    };
    let automatic = match file.encoding.filter(|_| !file.encoding_chosen) {
        Some(found) => format!("Automatic: {}", encoding_label(found)),
        None => "Automatic".to_owned(),
    };
    let mut items = vec![MenuItem::radio(automatic, !file.encoding_chosen, reread(None))];
    items.push(MenuItem::Separator);
    let listed = TEXT_ENCODINGS.iter().skip(1).filter_map(|choice| {
        Encoding::for_label(choice.id.as_bytes()).map(|encoding| (choice.label, encoding))
    });
    items.extend(listed.map(|(label, encoding)| {
        let checked = file.encoding_chosen && file.encoding == Some(encoding);
        MenuItem::radio(label, checked, reread(Some(encoding)))
    }));
    MenuItem::submenu("Text Encoding", items)
}

/// A submenu of the choices of the setting `key`, the one in effect checked; picking one holds
/// it for the session. Disabled, saying why, while the setting depends on another that is off
/// (the method while deinterlacing is).
fn session_menu(key: &'static str, title: &'static str, cx: &App) -> MenuItem {
    let Some(def) = registry::find(key) else { return MenuItem::submenu(title, Vec::new()) };
    let (Control::Dropdown(choices) | Control::Segmented(choices)) = def.control else {
        return MenuItem::submenu(title, Vec::new());
    };
    let effective = effects::effective(cx);
    let current = def.value(&effective);
    let items = choices.iter().map(|choice| {
        let checked = current == Value::Choice(choice.id);
        MenuItem::radio(choice.label, checked, move |_, cx| {
            effects::set(cx, key, Value::Choice(choice.id));
        })
    });
    let menu = MenuItem::submenu(title, items.collect());
    if def.is_enabled(&effective) {
        menu
    } else {
        menu.disabled(true).why_disabled("Deinterlacing is off")
    }
}

/// What Settings → Subtitles calls `encoding` ("Hebrew (Windows-1255)"), or its own name.
fn encoding_label(encoding: &'static Encoding) -> String {
    let listed = TEXT_ENCODINGS
        .iter()
        .find(|choice| Encoding::for_label(choice.id.as_bytes()) == Some(encoding));
    listed.map_or_else(|| encoding.name().to_owned(), |choice| choice.label.to_owned())
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

/// Visualizer ▸: Settings → Audio "Audio visualizer".
fn visualizer_menu(cx: &App) -> MenuItem {
    let shown = settings::get(cx).prefs.audio.visualizer;
    let items = Visualizer::ALL.into_iter().map(|choice| {
        MenuItem::radio(choice.label(), choice == shown, move |_, cx| {
            settings::update(cx, |s| s.prefs.audio.visualizer = choice);
        })
    });
    MenuItem::submenu("Visualizer", items.collect())
}
