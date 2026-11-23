//! Each control bar item drawn, and its entry in the overflow menu once it gives way.
//!
//! The seek bar (`seek_bar.rs`, S2.3), the time readouts (`readouts.rs`, S2.4) and the volume
//! control (`volume_slider.rs`, S2.5) are their own modules. The Audio and Subtitles buttons
//! open the track popover (`track_popover.rs`, S2.6) and the gear the settings panel
//! (`settings_panel.rs`, S2.7), and the List button the queue panel (`queue_panel.rs`, S3.2).

use std::time::Duration;

use gpui::{Action, AnyElement, App, ClickEvent, Context, Entity, SharedString, div, prelude::*};

use super::layout::{BUTTON, ICON, Item};
use super::play_button::{Face, PlayButton};
use crate::assets::icons;
use crate::engine::player::Player;
use crate::engine::tracks::TrackKind;
use crate::keymap;
use crate::keymap::actions as act;
use crate::keymap::menus;
use crate::main_window::MainWindow;
use crate::main_window::seeking::Way;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::menu::MenuItem;

/// What pressing an icon button does.
#[derive(Clone, Copy)]
enum Press {
    /// Runs a command.
    Run(&'static dyn Action),
    /// Opens (or closes) the track popover's tab of this kind.
    Tracks(TrackKind),
    /// Opens (or closes) the settings panel.
    Settings,
    /// Jumps [`MUSIC_SKIP`] this way.
    Skip(Way),
}

/// How far music mode's Back and Forward buttons jump, as their icons say (S8.1.2).
const MUSIC_SKIP: Duration = Duration::from_secs(15);

/// An icon button's look and what it does.
#[derive(Clone, Copy)]
struct ButtonItem {
    icon: &'static str,
    label: &'static str,
    press: Press,
    enabled: bool,
}

impl MainWindow {
    /// Draws `item`.
    pub(super) fn render_item(
        &self,
        item: Item,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(button) = self.button_item(item, cx) {
            return self.render_button(item, button, cx);
        }
        match item {
            Item::PlayPause => self.render_play_button(cx),
            Item::CurrentTime => self.render_current_time(p, cx),
            Item::Duration => self.render_duration(p, cx),
            Item::SeekBar => self.render_seek_bar(p, cx),
            _ => self.render_volume(p, cx),
        }
    }

    /// The overflow menu's entry for `item`, if it has one.
    pub(super) fn overflow_entry(&self, item: Item, cx: &mut Context<Self>) -> Option<MenuItem> {
        if item == Item::Volume {
            let player = self.loaded_player(cx)?;
            let label = mute_label(&player, cx);
            return Some(MenuItem::action(label, move |_, cx| {
                player.update(cx, Player::toggle_mute);
            }));
        }
        let ButtonItem { label, press, enabled, .. } = self.button_item(item, cx)?;
        let main = cx.entity().downgrade();
        let action = match press {
            Press::Run(action) => action,
            Press::Tracks(kind) => {
                let entry = MenuItem::action(label, move |window, cx| {
                    main.update(cx, |main, cx| main.open_track_popover(kind, window, cx)).ok();
                });
                return Some(entry.disabled(!enabled));
            }
            Press::Settings => {
                let entry = MenuItem::action(label, move |window, cx| {
                    main.update(cx, |main, cx| main.open_playback_settings(window, cx)).ok();
                });
                return Some(entry.disabled(!enabled));
            }
            Press::Skip(way) => {
                let entry = MenuItem::action(label, move |_, cx| {
                    main.update(cx, |main, cx| main.skip_by(way, MUSIC_SKIP, cx)).ok();
                });
                return Some(entry.disabled(!enabled));
            }
        };
        let entry = MenuItem::action(label, move |window, cx| {
            main.update(cx, |main, cx| main.perform(action, window, cx)).ok();
        })
        .disabled(!enabled);
        Some(match keymap::shortcut(action, cx) {
            Some(shortcut) => entry.shortcut(shortcut),
            None => entry,
        })
    }

    /// The icon button `item` is, if it is one.
    fn button_item(&self, item: Item, cx: &App) -> Option<ButtonItem> {
        let panel = |icon, label, press| {
            Some(ButtonItem { icon, label, press, enabled: self.has_active_media(cx) })
        };
        let (icon, action, enabled): (_, &'static dyn Action, _) = match item {
            Item::Previous => (icons::PREVIOUS, &act::PreviousItem, self.can_go_back(cx)),
            Item::Next => (icons::NEXT, &act::NextItem, self.can_go_next()),
            Item::Audio => return panel(icons::AUDIO, "Audio", Press::Tracks(TrackKind::Audio)),
            Item::Subtitles => {
                return panel(icons::SUBTITLES, "Subtitles", Press::Tracks(TrackKind::Subtitle));
            }
            Item::Settings => return panel(icons::SETTINGS, "Playback settings", Press::Settings),
            Item::Back15 => return panel(icons::BACK_15, "Back 15 s", Press::Skip(Way::Back)),
            Item::Forward15 => {
                return panel(icons::FORWARD_15, "Forward 15 s", Press::Skip(Way::Forward));
            }
            Item::List => (icons::LIST, &act::ToggleQueue, true),
            _ => return None,
        };
        let enabled = enabled && Self::handles(action);
        Some(ButtonItem { icon, label: menus::label(action), press: Press::Run(action), enabled })
    }

    fn render_button(&self, item: Item, button: ButtonItem, cx: &mut Context<Self>) -> AnyElement {
        let id = SharedString::from(format!("{item:?}"));
        let icon_button = IconButton::new(id, button.icon, button.label)
            .size(BUTTON, ICON)
            .selected(item == Item::List && self.queue_panel.is_open())
            .disabled(!button.enabled);
        match button.press {
            Press::Run(action) => {
                let icon_button =
                    icon_button.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.perform(action, window, cx);
                    }));
                match keymap::shortcut(action, cx) {
                    Some(shortcut) => icon_button.shortcut(shortcut).into_any_element(),
                    None => icon_button.into_any_element(),
                }
            }
            Press::Tracks(kind) => {
                let icon_button = icon_button.selected(self.track_popover.shows(kind)).on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.toggle_track_popover(kind, window, cx);
                    }),
                );
                // The popover tells a press on its button from an outside click by these bounds.
                let tracker = self.track_popover.button(kind).element();
                div().relative().flex_none().child(tracker).child(icon_button).into_any_element()
            }
            Press::Settings => {
                let icon_button = icon_button.selected(self.settings_panel.is_open()).on_click(
                    cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.toggle_settings_panel(window, cx);
                    }),
                );
                let tracker = self.settings_panel.button().element();
                div().relative().flex_none().child(tracker).child(icon_button).into_any_element()
            }
            Press::Skip(way) => icon_button
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.skip_by(way, MUSIC_SKIP, cx);
                }))
                .into_any_element(),
        }
    }

    fn render_play_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let face = self.loaded_player(cx).map_or(Face::Play, |player| {
            let player = player.read(cx);
            Face::of(player.state(), player.wants_to_play())
        });
        let shortcut = keymap::shortcut(&act::PlayPause, cx);
        let on_click = cx.listener(|this, _: &ClickEvent, window, cx| this.play_pause(window, cx));
        PlayButton::new(face, shortcut, on_click).into_any_element()
    }
}

fn mute_label(player: &Entity<Player>, cx: &App) -> &'static str {
    if player.read(cx).is_muted() { "Unmute" } else { "Mute" }
}
