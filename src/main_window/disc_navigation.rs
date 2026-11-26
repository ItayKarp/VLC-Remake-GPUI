//! Programs, titles and disc menus in the main window (S8.9): X plays the next program and
//! names it over the video, Shift+M goes to the disc's menu, and while a DVD or Blu-ray menu
//! shows, the arrow keys move its highlight and Enter or Space presses the button, as they do
//! on a disc player. libVLC 3 draws into the app's own picture, so a menu's buttons can't be
//! clicked; the keys reach them.

use gpui::{Action, Context};

use super::MainWindow;
use crate::engine::navigation::{MenuKey, Program};
use crate::engine::player::PlayerEvent;
use crate::keymap::actions as act;

/// What the video says when a menu comes up.
const MENU_HINT: &str = tr_noop!("Disc menu: arrow keys to move, Enter to choose");

impl MainWindow {
    /// X: plays the next program, naming it over the video.
    pub(super) fn next_program(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let next = player.update(cx, |player, _| player.next_program());
        let text = next.map_or_else(
            || tr!("No other program").to_owned(),
            |program| tr!("Program: {label}", label = program.label()),
        );
        self.show_osd(text, cx);
    }

    /// Plays the program with `id`, picked from Program ▸.
    pub(super) fn select_program(&mut self, id: i64, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        player.update(cx, |player, _| player.select_program(id));
        let label = player.read(cx).programs().all.iter().find(|p| p.id == id).map(Program::label);
        if let Some(label) = label {
            self.show_osd(tr!("Program: {label}", label), cx);
        }
    }

    /// Plays the title at `index`, picked from Title ▸.
    pub(super) fn select_title(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        player.update(cx, |player, _| player.select_title(index));
        let label = player.read(cx).titles().label(index);
        self.show_osd(label, cx);
    }

    /// Shift+M: goes to the disc's menu.
    pub(super) fn go_to_disc_menu(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        if !player.update(cx, |player, _| player.disc_menu()) {
            self.show_osd(tr!("This media has no menu"), cx);
        }
    }

    /// Enter: presses the highlighted button of the disc menu showing.
    pub(super) fn select_in_disc_menu(&mut self, cx: &mut Context<Self>) {
        self.press_menu_key(MenuKey::Activate, cx);
    }

    /// The menu key `action`'s key is while a disc menu shows: the arrows move, Space presses.
    pub(super) fn disc_menu_key(&mut self, action: &dyn Action, cx: &mut Context<Self>) -> bool {
        let keys: [(&dyn Action, MenuKey); 5] = [
            (&act::SeekBackward, MenuKey::Left),
            (&act::SeekForward, MenuKey::Right),
            (&act::VolumeUp, MenuKey::Up),
            (&act::VolumeDown, MenuKey::Down),
            (&act::PlayPause, MenuKey::Activate),
        ];
        let Some(&(_, key)) = keys.iter().find(|(bound, _)| bound.partial_eq(action)) else {
            return false;
        };
        self.press_menu_key(key, cx)
    }

    /// Presses `key` in the disc menu showing; `false` when none shows.
    fn press_menu_key(&mut self, key: MenuKey, cx: &mut Context<Self>) -> bool {
        let Some(player) = self.loaded_player(cx) else { return false };
        player.update(cx, |player, _| player.press_menu_key(key))
    }

    /// Says how to use a disc menu as one comes up: titles change as a menu opens.
    pub(super) fn on_disc_event(&mut self, event: &PlayerEvent, cx: &mut Context<Self>) {
        if *event != PlayerEvent::TitlesChanged {
            return;
        }
        if self.loaded_player(cx).is_some_and(|p| p.read(cx).titles().in_menu()) {
            self.show_osd(crate::i18n::text(MENU_HINT), cx);
        }
    }
}
