//! The sleep timer (S8.13.1): the settings panel's Sleep Timer stops playback after 15 to 90
//! minutes, minutes the user types, or at the end of the item playing. The sound fades out over
//! the last ten seconds first ([`rules::FADE`]), without moving the volume control, and comes
//! back to its level once playback stopped or the timer was turned off.
//!
//! It stops rather than pauses, even media already paused (the edge case in S8.13): someone
//! asleep wants it over, and Continue where you left off still picks it up next time. A timer
//! running past one item goes on through the queue; the end-of-item timer ends the queue at the
//! item playing when it ends. Timers last the session and are never saved.

pub(super) mod rules;

use std::time::{Duration, SystemTime};

use gpui::{AsyncApp, Context, Task, WeakEntity, Window};

use super::MainWindow;
use crate::engine::player::Player;
use rules::{Choice, Step, Timer};

/// How often a running timer looks at the clock, and while fading how often it lowers the
/// sound.
const TICK: Duration = Duration::from_secs(1);
const FADE_TICK: Duration = Duration::from_millis(200);

/// The sleep timer's state.
pub(super) struct SleepTimer {
    /// The dropdown entry picked.
    chosen: Choice,
    /// The timer running; `None` while off, or while Custom waits for its minutes.
    timer: Option<Timer>,
    /// Looks at the clock while a timer runs.
    _ticking: Option<Task<()>>,
}

impl Default for SleepTimer {
    fn default() -> Self {
        Self { chosen: Choice::Off, timer: None, _ticking: None }
    }
}

impl MainWindow {
    /// The dropdown entry the sleep timer shows.
    pub(super) fn sleep_choice(&self) -> Choice {
        self.sleep_timer.chosen
    }

    /// The line under the dropdown while a timer runs ([`rules::hint`]).
    pub(super) fn sleep_hint(&self, cx: &gpui::App) -> Option<String> {
        let timer = self.sleep_timer.timer?;
        Some(rules::hint(timer, timer.left(SystemTime::now(), self.item_left(cx))))
    }

    /// Picks `choice` in the dropdown: Off ends the timer, a length or the end of the item
    /// starts one, and Custom waits for minutes typed ([`Self::start_sleep_minutes`]).
    pub(super) fn pick_sleep_timer(&mut self, choice: Choice, cx: &mut Context<Self>) {
        match choice {
            Choice::Off => self.cancel_sleep_timer(cx),
            Choice::Minutes(minutes) => self.start_sleep_minutes(minutes, choice, cx),
            Choice::EndOfItem => self.arm_sleep_timer(Timer::EndOfItem, choice, cx),
            Choice::Custom => {
                self.cancel_sleep_timer(cx);
                self.sleep_timer.chosen = Choice::Custom;
            }
        }
    }

    /// Starts a timer of `minutes` from now, shown as `chosen` in the dropdown.
    pub(super) fn start_sleep_minutes(
        &mut self,
        minutes: u32,
        chosen: Choice,
        cx: &mut Context<Self>,
    ) {
        self.arm_sleep_timer(Timer::after_minutes(minutes, SystemTime::now()), chosen, cx);
    }

    /// The item playing ended: an end-of-item timer fires, ending the queue there; otherwise
    /// the queue goes on.
    pub(super) fn item_ended(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sleep_timer.timer == Some(Timer::EndOfItem) {
            self.fire_sleep_timer(cx);
        } else {
            self.advance(window, cx);
        }
    }

    fn arm_sleep_timer(&mut self, timer: Timer, chosen: Choice, cx: &mut Context<Self>) {
        self.cancel_sleep_timer(cx);
        let ticking = cx.spawn(async move |this, cx| keep_ticking(this, cx).await);
        self.sleep_timer = SleepTimer { chosen, timer: Some(timer), _ticking: Some(ticking) };
        cx.notify();
    }

    /// Ends the timer, bringing the sound back to its level. Its loop sees that at its next
    /// look and ends; it may be the caller, so it isn't dropped here.
    fn cancel_sleep_timer(&mut self, cx: &mut Context<Self>) {
        self.sleep_timer.chosen = Choice::Off;
        self.sleep_timer.timer = None;
        self.set_sleep_fade(1.0, cx);
        cx.notify();
    }

    /// Fades or fires as the clock says; returns how long until the next look, or `None` once
    /// the timer is over.
    fn tick_sleep_timer(&mut self, cx: &mut Context<Self>) -> Option<Duration> {
        let timer = self.sleep_timer.timer?;
        // The hint under the dropdown counts down.
        cx.notify();
        match timer.step(SystemTime::now(), self.item_left(cx)) {
            Step::Wait => {
                self.set_sleep_fade(1.0, cx);
                Some(TICK)
            }
            Step::Fade(loudness) => {
                self.set_sleep_fade(loudness, cx);
                Some(FADE_TICK)
            }
            Step::Fire => {
                self.fire_sleep_timer(cx);
                None
            }
        }
    }

    /// Stops playback, ends the timer and says so.
    fn fire_sleep_timer(&mut self, cx: &mut Context<Self>) {
        tracing::info!("sleep timer: stopping playback");
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, Player::stop);
        }
        self.cancel_sleep_timer(cx);
        self.show_osd(tr!("Sleep timer: playback stopped"), cx);
    }

    fn set_sleep_fade(&self, loudness: f32, cx: &mut Context<Self>) {
        if let Some((player, _)) = &self.player {
            player.update(cx, |player, cx| player.set_fade(loudness, cx));
        }
    }

    /// How much of the item playing is left; `None` while nothing plays or its length isn't
    /// known (a live stream).
    fn item_left(&self, cx: &gpui::App) -> Option<Duration> {
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        let playing = player.state().is_active();
        let duration = player.duration().filter(|_| playing)?;
        Some(duration.saturating_sub(player.position()))
    }
}

/// Looks at the clock as often as the timer asks, until the window goes away or the timer ends.
async fn keep_ticking(this: WeakEntity<MainWindow>, cx: &mut AsyncApp) {
    while let Ok(Some(wait)) = this.update(cx, MainWindow::tick_sleep_timer) {
        cx.background_executor().timer(wait).await;
    }
}
