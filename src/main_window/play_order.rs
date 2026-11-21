//! Repeat, Shuffle and the end of the queue (S3.3) in the main window. The queue header's
//! buttons, the video menu's Playlist ▸ and the commands all change the queue's own Repeat and
//! Shuffle through here, so they always agree (S3.3.4); each change is said over the video.
//! "Stop after each item" and "When playback finishes" are settings (Settings → Playback, S5.7),
//! which the same menu edits until that screen exists.

use gpui::{App, Context, WeakEntity, Window};

use super::MainWindow;
use crate::assets::icons;
use crate::queue::Repeat;
use crate::settings::{self, WhenFinished};
use crate::ui::menu::MenuItem;

impl MainWindow {
    /// Moves Repeat to its next mode: Off, All, One.
    pub(super) fn cycle_repeat(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.set_repeat(self.queue.repeat().cycled(), cx);
    }

    /// Switches Shuffle on or off.
    pub(super) fn toggle_shuffle(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let on = !self.queue.is_shuffled();
        self.queue.set_shuffle(on);
        self.show_osd(if on { "Shuffle: On" } else { "Shuffle: Off" }, cx);
        cx.notify();
    }

    /// Sets the repeat mode.
    fn set_repeat(&mut self, repeat: Repeat, cx: &mut Context<Self>) {
        self.queue.set_repeat(repeat);
        self.show_osd(format!("Repeat: {}", repeat.label()), cx);
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
            MenuItem::submenu("Repeat", repeats.collect()).icon(icons::REPEAT),
            MenuItem::check("Shuffle", self.queue.is_shuffled(), move |window, cx| {
                shuffle.update(cx, |this, cx| this.toggle_shuffle(window, cx)).ok();
            }),
            MenuItem::Separator,
            MenuItem::check("Stop After Each Item", stop_after_each, move |_, cx| {
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
    MenuItem::submenu("When Playback Finishes", choices.collect())
}

/// The queue header's Repeat button: its icon, whether it shows as on, and its tooltip. The ON
/// states aren't designed; All and One show as on, One with a 1 in the arrows.
pub(super) fn repeat_button_look(repeat: Repeat) -> (&'static str, bool, &'static str) {
    match repeat {
        Repeat::Off => (icons::REPEAT, false, "Repeat: Off"),
        Repeat::All => (icons::REPEAT, true, "Repeat: All"),
        Repeat::One => (icons::REPEAT_ONE, true, "Repeat: One"),
    }
}
