//! Each layout item drawn in a bar, and its entry in the overflow menu once it gives way.
//!
//! The seek bar (`seek_bar.rs`, S2.3), the time readouts (`readouts.rs`, S2.4) and the volume
//! control (`volume_slider.rs`, S2.5) are their own modules. The Audio and Subtitles buttons
//! open the track popover (`track_popover.rs`, S2.6) and the gear the settings panel
//! (`settings_panel.rs`, S2.7), and the List button the queue panel (`queue_panel.rs`, S3.2).
//! What the icon buttons look like and do is `buttons.rs`; the names are `names.rs`.

use gpui::{AnyElement, App, ClickEvent, Context, Entity, SharedString, Window, div, prelude::*};

use super::buttons::{ButtonItem, LibraryPlace, Press};
use super::layout::{BUTTON, ICON};
use super::play_button::{Face, PlayButton};
use crate::engine::player::Player;
use crate::keymap;
use crate::keymap::actions as act;
use crate::main_window::MainWindow;
use crate::navigation::route::Route;
use crate::player_layout::Item;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::menu::MenuItem;

/// Which bar an item is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::main_window) enum Place {
    /// The title bar: smaller icons, and presses don't drag the window.
    TitleBar,
    /// The control bar.
    ControlBar,
}

/// The title bar's buttons: 28 px square with 18 px icons.
const TITLE_BAR_ICON: f32 = 18.;

impl MainWindow {
    /// Draws `item` in the bar `place`.
    pub(in crate::main_window) fn render_item(
        &self,
        item: Item,
        place: Place,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(button) = self.button_item(item, cx) {
            return self.render_button(item, button, place, cx);
        }
        match item {
            Item::PlayPause => self.render_play_button(cx),
            Item::CurrentTime => self.render_current_time(p, cx),
            Item::Duration => self.render_duration(p, cx),
            Item::SeekBar => self.render_seek_bar(p, cx),
            Item::Volume => self.render_volume(p, cx),
            _ => self.render_named_item(item, place, p, cx),
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
        let entry = MenuItem::action(label, move |window, cx| {
            main.update(cx, |main, cx| main.choose(press, window, cx)).ok();
        })
        .disabled(!enabled);
        let shortcut = match press {
            Press::Run(action) | Press::Dispatch(action) => keymap::shortcut(action, cx),
            _ => None,
        };
        Some(entry.maybe_shortcut(shortcut))
    }

    /// Does what choosing a button's overflow menu entry does: as pressing it, except that the
    /// panels open rather than toggle.
    fn choose(&mut self, press: Press, window: &mut Window, cx: &mut Context<Self>) {
        match press {
            Press::Tracks(kind) => self.open_track_popover(kind, window, cx),
            Press::Settings => self.open_playback_settings(window, cx),
            _ => self.press(press, window, cx),
        }
    }

    /// Does what pressing a button with `press` does.
    fn press(&mut self, press: Press, window: &mut Window, cx: &mut Context<Self>) {
        match press {
            Press::Run(action) => {
                self.perform(action, window, cx);
            }
            Press::Dispatch(action) => window.dispatch_action(action.boxed_clone(), cx),
            Press::Tracks(kind) => self.toggle_track_popover(kind, window, cx),
            Press::Settings => self.toggle_settings_panel(window, cx),
            Press::Skip(way, by) => self.skip_by(way, by, cx),
            Press::Tab(tab) => self.toggle_list_tab(tab, window, cx),
            Press::Library(place) => {
                let route = match place {
                    LibraryPlace::Home => Route::Home,
                    LibraryPlace::Playlists => Route::Playlists(None),
                };
                self.show_library_route(route, window, cx);
            }
            Press::Replay => self.restart(cx),
            Press::Favorite => self.toggle_playing_favorite(cx),
            Press::Hdr => self.toggle_session_hdr(cx),
            Press::Zoom => self.step_session_zoom(cx),
            Press::RightSidebar => self.toggle_right_sidebar(window, cx),
        }
    }

    /// The icon button `button` for `item` in the bar `place`: its look, its state and what it
    /// does.
    fn render_button(
        &self,
        item: Item,
        button: ButtonItem,
        place: Place,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = button.selected
            || match button.press {
                Press::Run(_) => item == Item::List && self.queue_panel.is_open(),
                Press::Tracks(kind) => self.track_popover.shows(kind),
                Press::Settings => self.settings_panel.is_open(),
                Press::Tab(tab) => self.queue_panel.shows(tab),
                _ => false,
            };
        let press = button.press;
        let id = SharedString::from(format!("{item:?}"));
        let icon_button = IconButton::new(id, button.icon, button.label);
        let icon_button = match place {
            Place::TitleBar => icon_button.size(BUTTON, TITLE_BAR_ICON).in_title_bar(),
            Place::ControlBar => icon_button.size(BUTTON, ICON),
        };
        let icon_button = icon_button.selected(selected).disabled(!button.enabled).on_click(
            cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.press(press, window, cx);
            }),
        );
        let shortcut = match press {
            Press::Run(action) | Press::Dispatch(action) => keymap::shortcut(action, cx),
            _ => None,
        };
        let icon_button = match shortcut {
            Some(shortcut) => icon_button.shortcut(shortcut),
            None => icon_button,
        };
        // The panels tell a press on their button from an outside click by these bounds.
        let tracker = match press {
            Press::Tracks(kind) => self.track_popover.button(kind).element(),
            Press::Settings if item == Item::Settings => self.settings_panel.button().element(),
            _ => return icon_button.into_any_element(),
        };
        div().relative().flex_none().child(tracker).child(icon_button).into_any_element()
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
