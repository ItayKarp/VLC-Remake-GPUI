//! Repeat, Shuffle and the end of the queue (S3.3) in the main window. The queue header's
//! buttons, the video menu's Playlist ▸ and the commands all change the queue's own Repeat and
//! Shuffle through here, so they always agree (S3.3.4); each change is said over the video.
//! "Stop after each item" and "When playback finishes" are settings, which Settings → Playback
//! (S5.7) and the same menu edit.
//!
//! Settings → Playback's Repeat and Shuffle rows edit the queue's own state too: the window copies
//! it into the settings' [`LiveState`] mirror whenever it changes, and applies a change made
//! there back to the queue. Whichever side changed last wins.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, Context, WeakEntity, Window};

use super::MainWindow;
use crate::assets::icons;
use crate::queue::Repeat;
use crate::settings::{self, LiveState, SettingsStore, WhenFinished};
use crate::ui::menu::MenuItem;

impl MainWindow {
    /// Keeps the queue's Repeat and Shuffle and Settings → Playback's rows in step, starting
    /// from the queue's. `synced` is the state both last agreed on, so each side only acts on
    /// its own changes.
    pub(super) fn follow_play_order(&mut self, cx: &mut Context<Self>) {
        let synced = Rc::new(Cell::new(settings::get(cx).live));
        self.publish_play_order(&synced, cx);
        let published = Rc::clone(&synced);
        cx.observe_self(move |this, cx| this.publish_play_order(&published, cx)).detach();
        cx.observe_global::<SettingsStore>(move |this, cx| this.take_play_order(&synced, cx))
            .detach();
    }

    /// The queue's Repeat and Shuffle as the settings mirror them.
    fn play_order(&self) -> LiveState {
        LiveState { repeat: self.queue.repeat(), shuffle: self.queue.is_shuffled() }
    }

    /// The queue's Repeat or Shuffle changed: shows it in Settings.
    fn publish_play_order(&mut self, synced: &Cell<LiveState>, cx: &mut Context<Self>) {
        let current = self.play_order();
        if current != synced.get() {
            synced.set(current);
            settings::update(cx, |s| s.live = current);
        }
    }

    /// Settings → Playback changed Repeat or Shuffle: applies it to the queue.
    fn take_play_order(&mut self, synced: &Cell<LiveState>, cx: &mut Context<Self>) {
        let wanted = settings::get(cx).live;
        if wanted == synced.get() {
            return;
        }
        synced.set(wanted);
        self.queue.set_repeat(wanted.repeat);
        if wanted.shuffle != self.queue.is_shuffled() {
            self.queue.set_shuffle(wanted.shuffle);
        }
        cx.notify();
    }

    /// Moves Repeat to its next mode: Off, All, One.
    pub(super) fn cycle_repeat(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.set_repeat(self.queue.repeat().cycled(), cx);
    }

    /// Switches Shuffle on or off.
    pub(super) fn toggle_shuffle(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let on = !self.queue.is_shuffled();
        self.queue.set_shuffle(on);
        self.show_osd(if on { tr!("Shuffle: On") } else { tr!("Shuffle: Off") }, cx);
        cx.notify();
    }

    /// Sets the repeat mode.
    fn set_repeat(&mut self, repeat: Repeat, cx: &mut Context<Self>) {
        self.queue.set_repeat(repeat);
        let (_, _, text) = repeat_button_look(repeat);
        self.show_osd(text, cx);
        cx.notify();
    }

    /// Repeat ▸, Shuffle, Stop After Each Item and When Playback Finishes ▸, for the video
    /// menu's Playlist ▸.
    pub(super) fn play_order_items(&self, main: &WeakEntity<Self>, cx: &App) -> Vec<MenuItem> {
        let repeats = Repeat::ALL.iter().map(|&repeat| {
            let main = main.clone();
            MenuItem::radio(repeat.label(), repeat == self.queue.repeat(), move |_, cx| {
                main.update(cx, |this, cx| this.set_repeat(repeat, cx)).ok();
            })
        });
        let shuffle = main.clone();
        let prefs = &settings::get(cx).prefs.player;
        let stop_after_each = prefs.stop_after_each_item;
        vec![
            MenuItem::submenu(tr!("Repeat"), repeats.collect()).icon(icons::REPEAT),
            MenuItem::check(tr!("Shuffle"), self.queue.is_shuffled(), move |window, cx| {
                shuffle.update(cx, |this, cx| this.toggle_shuffle(window, cx)).ok();
            }),
            MenuItem::Separator,
            MenuItem::check(tr!("Stop After Each Item"), stop_after_each, move |_, cx| {
                settings::update(cx, |s| s.prefs.player.stop_after_each_item = !stop_after_each);
            }),
            when_finished_menu(prefs.when_finished),
        ]
    }
}

/// When Playback Finishes ▸: what happens after the queue's last item.
fn when_finished_menu(current: WhenFinished) -> MenuItem {
    let choices = WhenFinished::ALL.iter().map(|&choice| {
        MenuItem::radio(choice.label(), choice == current, move |_, cx| {
            settings::update(cx, |s| s.prefs.player.when_finished = choice);
        })
    });
    MenuItem::submenu(tr!("When Playback Finishes"), choices.collect())
}

/// The queue header's Repeat button: its icon, whether it shows as on, and its tooltip. The ON
/// states aren't designed; All and One show as on, One with a 1 in the arrows.
pub(super) fn repeat_button_look(repeat: Repeat) -> (&'static str, bool, &'static str) {
    match repeat {
        Repeat::Off => (icons::REPEAT, false, tr!("Repeat: Off")),
        Repeat::All => (icons::REPEAT, true, tr!("Repeat: All")),
        Repeat::One => (icons::REPEAT_ONE, true, tr!("Repeat: One")),
    }
}

#[cfg(test)]
mod tests {
    use gpui::{TestAppContext, VisualTestContext};

    use super::*;
    use crate::main_window::tests::{main_view, open};

    fn live(cx: &mut VisualTestContext) -> LiveState {
        cx.update(|_, cx| settings::get(cx).live)
    }

    #[gpui::test]
    fn the_queues_buttons_show_in_settings(cx: &mut TestAppContext) {
        let cx = open(cx);
        let main = main_view(cx);
        main.update_in(cx, |main, window, cx| {
            main.cycle_repeat(window, cx);
            main.toggle_shuffle(window, cx);
        });
        cx.run_until_parked();
        assert_eq!(live(cx), LiveState { repeat: Repeat::All, shuffle: true });
    }

    #[gpui::test]
    fn settings_change_the_queues_own_repeat_and_shuffle(cx: &mut TestAppContext) {
        let cx = open(cx);
        cx.update(|_, cx| {
            settings::update(cx, |s| s.live = LiveState { repeat: Repeat::One, shuffle: true });
        });
        cx.run_until_parked();
        let main = main_view(cx);
        main.read_with(cx, |main, _| {
            assert_eq!(main.queue.repeat(), Repeat::One);
            assert!(main.queue.is_shuffled());
        });
        main.update_in(cx, MainWindow::cycle_repeat);
        cx.run_until_parked();
        assert_eq!(live(cx).repeat, Repeat::Off, "the button wins after a Settings change");
    }
}
