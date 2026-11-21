//! Playback speed (S2.7.2–S2.7.4): the settings panel's Speed controls, and `[`, `]` and `=`
//! for slower, faster and normal speed, each saying the new speed over the video ("Speed
//! 1.25×").
//!
//! A launch starts at Settings → Playback "Default playback speed"
//! (`prefs.player.default_speed`, normal speed by default; its row comes with S5.7). A speed the
//! user picks then lasts for the rest of the session, carried from item to item, and is never
//! saved. A live stream (media that can't seek) plays at the speed it gets: its controls are
//! disabled and the keys do nothing. libVLC keeps the pitch at any speed (its time-stretch audio
//! filter is on by default), and speed stops at 4×, past which it would mute the audio.

pub(super) mod rules;

use gpui::{App, Context, Entity, Window};

use super::MainWindow;
use crate::engine::player::Player;
use crate::engine::values::Rate;
use crate::settings;
use rules::SpeedChange;

impl MainWindow {
    /// The player whose speed can change: open media that isn't a live stream.
    pub(super) fn speed_player(&self, cx: &App) -> Option<Entity<Player>> {
        self.loaded_player(cx).filter(|player| {
            let player = player.read(cx);
            player.state().is_active() && player.is_seekable() != Some(false)
        })
    }

    /// Moves the speed for a key, and says so over the video.
    pub(super) fn change_speed(
        &mut self,
        change: SpeedChange,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(player) = self.speed_player(cx) else { return };
        let speed = rules::changed(player.read(cx).rate().get(), change);
        self.set_speed(speed, cx);
        self.show_osd(rules::message(speed), cx);
    }

    /// Plays at `speed`, clamped to the range the player allows. The panel's controls follow
    /// when they're next drawn.
    pub(super) fn set_speed(&mut self, speed: f32, cx: &mut Context<Self>) {
        if let Some(player) = self.speed_player(cx) {
            player.update(cx, |player, cx| player.set_rate(Rate::new(speed), cx));
            cx.notify();
        }
    }

    /// Starts a new player at "Default playback speed".
    pub(super) fn start_at_default_speed(player: &Entity<Player>, cx: &mut Context<Self>) {
        let rate = Rate::new(settings::get(cx).prefs.player.default_speed);
        if rate != player.read(cx).rate() {
            player.update(cx, |player, cx| player.set_rate(rate, cx));
        }
    }
}
