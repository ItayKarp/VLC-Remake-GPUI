//! What the window shows around the video (S2.8), decided without GPUI: the layout, which bars
//! show, when the pointer counts as having moved, and the idle timer that hides the controls.

use std::time::{Duration, Instant};

use crate::settings::AutoHide;

/// How far the pointer must move to wake the controls. A resting mouse, or a hand on the
/// trackpad, jitters a pixel or two; that must not bring the controls back (S2.8 edge case).
const WAKE_DISTANCE: f32 = 4.;

/// How the window lays out the video and its bars.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// The title bar above and the control bar below the inset video: nothing hides.
    Docked,
    /// The inset video fills the window (Hide UI, or auto-hide in a window); shown bars float
    /// over it, so showing and hiding them never resizes the video.
    Floating,
    /// Fullscreen: the video edge to edge, the control bar floating over it while shown.
    Fullscreen,
}

/// What decides the window's chrome.
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent facts about the window, read by one decision"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Situation {
    /// The window is fullscreen.
    pub fullscreen: bool,
    /// Hide UI mode is on (S2.8.2).
    pub hide_ui: bool,
    /// Video (or the audio placeholder) shows; with the empty state nothing hides.
    pub video: bool,
    /// The pointer rested for the "Hide after" delay.
    pub idle: bool,
    /// Something keeps the controls up (S2.8.3).
    pub held: bool,
    /// The "Show interface controls in fullscreen" key asked for them.
    pub summoned: bool,
    /// Full library mode (S4.10.2): the library fills the window, so there is no control bar
    /// and nothing to hide it over.
    pub full_library: bool,
    /// The user's choices.
    pub prefs: Prefs,
}

/// The settings the chrome follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// "Auto-hide the controls".
    pub auto_hide: AutoHide,
    /// "Show controls in fullscreen".
    pub show_in_fullscreen: bool,
    /// "Show window decorations" (S5.4.1): off, the player has no title bar.
    pub decorations: bool,
}

/// The window's chrome for a [`Situation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chrome {
    /// Where the video and bars go.
    pub layout: Layout,
    /// Whether the title bar shows.
    pub title_bar: bool,
    /// Whether the control bar (and what hangs off it: popovers, the seek card) shows.
    pub controls: bool,
}

impl Situation {
    /// Whether the controls hide by themselves here.
    pub fn auto_hides(self) -> bool {
        let applies = match self.prefs.auto_hide {
            AutoHide::Always => true,
            AutoHide::InFullscreenOnly => self.fullscreen,
            AutoHide::Never => false,
        };
        self.video && applies && !self.full_library
    }

    /// Whether the idle timer runs: the controls, or at least the cursor, hide after a rest.
    pub fn idles(self) -> bool {
        self.auto_hides() || (self.video && self.hide_ui && !self.full_library)
    }

    /// The layout, and which bars show.
    pub fn chrome(self) -> Chrome {
        if self.full_library {
            let layout = if self.fullscreen { Layout::Fullscreen } else { Layout::Docked };
            return Chrome { layout, title_bar: !self.fullscreen, controls: false };
        }
        let layout = if self.fullscreen {
            Layout::Fullscreen
        } else if self.hide_ui || (self.auto_hides() && self.prefs.auto_hide == AutoHide::Always) {
            Layout::Floating
        } else {
            Layout::Docked
        };
        let controls = self.controls_show();
        let title_bar = controls && !self.fullscreen && self.prefs.decorations;
        Chrome { layout, title_bar, controls }
    }

    fn controls_show(self) -> bool {
        if self.hide_ui {
            return false;
        }
        if self.fullscreen && !self.prefs.show_in_fullscreen && !self.summoned {
            return false;
        }
        !self.auto_hides() || self.held || self.summoned || !self.idle
    }
}

/// Whether the pointer at window height `y` is over a bar: the control bar along the bottom
/// (`bar_height` tall), or the title bar along the top (`title_height`) when it shows.
pub fn over_bars(y: f32, window_height: f32, bar_height: f32, title_height: Option<f32>) -> bool {
    y >= window_height - bar_height || title_height.is_some_and(|height| y < height)
}

/// Where the pointer last counted as moving, so jitter doesn't.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerAnchor(Option<(f32, f32)>);

impl PointerAnchor {
    /// Whether moving to `(x, y)` is a real move; if so it becomes the new anchor.
    pub fn moved_to(&mut self, x: f32, y: f32) -> bool {
        let moved = self.0.is_none_or(|(ax, ay)| (x - ax).hypot(y - ay) >= WAKE_DISTANCE);
        if moved {
            self.0 = Some((x, y));
        }
        moved
    }

    /// Forgets the anchor: the pointer left the window.
    pub fn clear(&mut self) {
        self.0 = None;
    }
}

/// The "Hide after" countdown, on the monotonic clock (wall-clock jumps don't touch it). One
/// timer task runs at a time; waking only moves the deadline it sleeps toward.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IdleTimer {
    countdown: Countdown,
    /// A timer task is sleeping toward the deadline.
    running: bool,
    /// Last frame's hold.
    was_held: bool,
}

/// Where the countdown stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Countdown {
    /// Not counting: the controls stay.
    #[default]
    Off,
    /// The controls hide at this instant.
    Until(Instant),
    /// The pointer rested: the controls are hidden.
    Rested,
}

/// What the timer task does after a sleep.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tick {
    /// The deadline moved: sleep this much longer.
    Sleep(Duration),
    /// The rest is over: hide the controls and the cursor, and stop.
    Hide,
    /// Nothing to hide (a hold keeps the controls up, or the timer was cancelled): stop.
    Stop,
}

impl IdleTimer {
    /// Whether the pointer rested for the delay.
    pub fn is_idle(&self) -> bool {
        self.countdown == Countdown::Rested
    }

    /// Activity at `now`: the controls show for `delay` from now. Returns the sleep a new timer
    /// task needs, or `None` when one is already running.
    pub fn wake(&mut self, now: Instant, delay: Duration) -> Option<Duration> {
        self.countdown = Countdown::Until(now + delay);
        let start = !self.running;
        self.running = true;
        start.then_some(delay)
    }

    /// Stops counting: the controls no longer hide (a docked window, the empty state).
    pub fn cancel(&mut self) {
        self.countdown = Countdown::Off;
    }

    /// The timer task woke at `now`; `held` says whether something keeps the controls up.
    pub fn tick(&mut self, now: Instant, held: bool) -> Tick {
        if let Countdown::Until(deadline) = self.countdown
            && now < deadline
        {
            return Tick::Sleep(deadline - now);
        }
        self.running = false;
        let due = matches!(self.countdown, Countdown::Until(_));
        if !due || held {
            self.countdown = Countdown::Off;
            return Tick::Stop;
        }
        self.countdown = Countdown::Rested;
        Tick::Hide
    }

    /// Follows the hold each frame. Returns `true` when a hold just ended, so the delay starts
    /// over from now (resuming play, closing a popover, ending a drag).
    pub fn hold_ended(&mut self, held: bool) -> bool {
        let ended = self.was_held && !held;
        self.was_held = held;
        ended
    }
}

#[cfg(test)]
mod tests;
