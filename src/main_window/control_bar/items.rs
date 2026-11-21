//! Each control bar item drawn, and its entry in the overflow menu once it gives way.
//!
//! The seek bar (`seek_bar.rs`, S2.3) and the time readouts (`readouts.rs`, S2.4) are their own
//! modules. The volume level shows the player's state but can't be used yet: S2.5 (the wedge
//! slider) makes it interactive. The Settings and List buttons are disabled until their panels
//! exist (S2.7, Stage 3): the light theme's faded gear means exactly that.

use gpui::{
    Action, AnyElement, App, ClickEvent, Context, Div, Entity, SharedString, div, prelude::*, px,
    relative,
};

use super::layout::{BUTTON, Item, LEVEL_GAP, LEVEL_WIDTH};
use super::play_button::{Face, PlayButton};
use crate::assets::icons;
use crate::engine::player::Player;
use crate::keymap;
use crate::keymap::actions as act;
use crate::keymap::menus;
use crate::main_window::MainWindow;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::menu::MenuItem;

/// The icons' size inside the bar's buttons.
const ICON: f32 = 20.;
/// The volume level's height.
const LEVEL_HEIGHT: f32 = 10.;

/// An icon button's look and what it runs.
#[derive(Clone, Copy)]
struct ButtonItem {
    icon: &'static str,
    label: &'static str,
    action: &'static dyn Action,
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
            return Self::render_button(item, button, cx);
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
        let ButtonItem { label, action, enabled, .. } = self.button_item(item, cx)?;
        let main = cx.entity().downgrade();
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
        let active =
            self.loaded_player(cx).is_some_and(|player| player.read(cx).state().is_active());
        let (icon, action, enabled): (_, &'static dyn Action, _) = match item {
            Item::Previous => (icons::PREVIOUS, &act::PreviousItem, self.can_go_back(cx)),
            Item::Next => (icons::NEXT, &act::NextItem, self.can_go_next()),
            Item::Audio => (icons::AUDIO, &act::CycleAudioTrack, active),
            Item::Subtitles => (icons::SUBTITLES, &act::CycleSubtitleTrack, active),
            Item::Settings => (icons::SETTINGS, &act::OpenSettings, false),
            Item::List => (icons::LIST, &act::ToggleQueue, true),
            _ => return None,
        };
        let label = if item == Item::Settings { "Playback settings" } else { menus::label(action) };
        Some(ButtonItem { icon, label, action, enabled: enabled && Self::handles(action) })
    }

    fn render_button(item: Item, button: ButtonItem, cx: &mut Context<Self>) -> AnyElement {
        let action = button.action;
        let mut icon_button =
            IconButton::new(SharedString::from(format!("{item:?}")), button.icon, button.label)
                .size(BUTTON, ICON)
                .disabled(!button.enabled)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.perform(action, window, cx);
                }));
        if let Some(shortcut) = keymap::shortcut(action, cx) {
            icon_button = icon_button.shortcut(shortcut);
        }
        icon_button.into_any_element()
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

    /// The speaker, which mutes, and the volume level.
    fn render_volume(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let player = self.loaded_player(cx);
        let label = player.as_ref().map_or("Mute", |player| mute_label(player, cx));
        let level = player.as_ref().map_or(0., |player| {
            let player = player.read(cx);
            if player.is_muted() { 0. } else { player.volume().get() / player.max_volume().get() }
        });
        let speaker = IconButton::new("speaker", icons::VOLUME, label)
            .size(BUTTON, ICON)
            .disabled(player.is_none())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(player) = this.loaded_player(cx) {
                    player.update(cx, Player::toggle_mute);
                }
            }));
        div()
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(LEVEL_GAP))
            .child(speaker)
            .child(volume_level(p, level))
            .into_any_element()
    }
}

fn mute_label(player: &Entity<Player>, cx: &App) -> &'static str {
    if player.read(cx).is_muted() { "Unmute" } else { "Mute" }
}

fn volume_level(p: &Palette, level: f32) -> Div {
    div()
        .w(px(LEVEL_WIDTH))
        .h(px(LEVEL_HEIGHT))
        .flex_none()
        .rounded(px(2.))
        .bg(p.sunken)
        .child(div().w(relative(level.clamp(0., 1.))).h_full().rounded(px(2.)).bg(p.volume))
}
