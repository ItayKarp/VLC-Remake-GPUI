//! Auto-hiding controls (S2.8.1, S2.8.3, S2.8.4). While video plays where Settings → Interface
//! "Auto-hide the controls" applies (fullscreen by default), the bars hide once the pointer
//! rests for "Hide after" (3 s), and the cursor hides with them. A real pointer move over the
//! window brings them back; jitter and moves on another screen don't.
//!
//! They stay while playback isn't running (paused, unless "Keep controls while paused" is off),
//! the pointer is over a bar, a popover or menu is open, the seek bar or volume wedge is being
//! dragged, or keyboard focus is on one of their controls. When a hold ends the delay starts
//! over. Fullscreen drops the title bar; with "Show controls in fullscreen" off the control bar
//! only shows there when the I key asks for it, and Escape leaves fullscreen.
//!
//! The rules are in `rules.rs`; this module feeds them the window's state.

pub(super) mod rules;

use std::time::{Duration, Instant};

use gpui::{
    App, Bounds, Context, DispatchPhase, Entity, IntoElement, MouseMoveEvent, Pixels, Point,
    Window, canvas, point, prelude::*,
};

use self::rules::{Chrome, IdleTimer, PointerAnchor, Prefs, Situation, Tick};
use super::control_bar::CONTROL_BAR_HEIGHT;
use super::video_menu::VideoMenu;
use super::{MainWindow, TITLE_BAR_HEIGHT};
use crate::cursor;
use crate::engine::state::PlaybackState;
use crate::settings;
use crate::ui::popover::BoundsTracker;

/// The auto-hide state of the main window.
#[derive(Default)]
pub struct AutoHide {
    timer: IdleTimer,
    /// The pointer's position while it is over the window.
    pointer: Option<Point<Pixels>>,
    anchor: PointerAnchor,
    /// Hide UI mode (S2.8.2): only the video shows.
    pub(super) hide_ui: bool,
    /// The video's right-click menu, while it's open.
    pub(super) video_menu: Option<VideoMenu>,
    /// The video's bounds, where the menu opens from the keyboard (S2.9).
    pub(super) video_area: BoundsTracker,
    /// The I key asked for the controls; they show until the pointer next rests.
    summoned: bool,
    seen: Seen,
}

/// What the last frame had, to act on changes.
#[derive(Default)]
struct Seen {
    /// The idle timer was meant to run.
    idles: bool,
    /// The window was fullscreen.
    fullscreen: bool,
    /// What the native title bar (the macOS traffic lights) was last set to.
    native_title_bar: Option<bool>,
}

impl MainWindow {
    /// Where the window stands for the auto-hide rules.
    fn situation(&self, window: &Window, cx: &App) -> Situation {
        let interface = &settings::get(cx).prefs.interface;
        Situation {
            fullscreen: window.is_fullscreen(),
            hide_ui: self.auto_hide.hide_ui,
            video: self.showing_video(cx).is_some(),
            idle: self.auto_hide.timer.is_idle(),
            held: self.controls_held(window, cx),
            summoned: self.auto_hide.summoned,
            prefs: Prefs {
                auto_hide: interface.auto_hide_controls,
                show_in_fullscreen: interface.show_controls_in_fullscreen,
            },
        }
    }

    /// The layout and which bars show now.
    pub(super) fn chrome(&self, window: &Window, cx: &App) -> Chrome {
        self.situation(window, cx).chrome()
    }

    /// Brings the frame up to date before it renders: restarts the delay when the timer starts
    /// being needed or a hold ends, shows or hides the traffic lights, and closes what hangs off
    /// a bar that no longer shows.
    pub(super) fn sync_chrome(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Chrome {
        // macOS swallows the key-up of a key that switched fullscreen while it animates, so the
        // held-key gate would ignore that key's next press (Escape out of Hide UI, F again).
        if window.is_fullscreen() != self.auto_hide.seen.fullscreen {
            self.auto_hide.seen.fullscreen = window.is_fullscreen();
            self.repeat.release_all();
        }
        let situation = self.situation(window, cx);
        let started = situation.idles() && !self.auto_hide.seen.idles;
        self.auto_hide.seen.idles = situation.idles();
        if self.auto_hide.timer.hold_ended(situation.held) || started {
            self.wake_controls(window, cx);
        } else if !situation.idles() {
            self.auto_hide.timer.cancel();
        }
        let chrome = self.chrome(window, cx);
        self.sync_native_title_bar(chrome.title_bar || window.is_fullscreen(), window);
        if !chrome.controls {
            self.control_bar.close_overflow();
            self.close_popovers(window, cx);
        }
        if !chrome.title_bar {
            self.menu = None;
        }
        chrome
    }

    /// Shows or hides the native title bar (macOS's traffic lights) with ours.
    fn sync_native_title_bar(&mut self, shown: bool, window: &Window) {
        if self.auto_hide.seen.native_title_bar != Some(shown) {
            self.auto_hide.seen.native_title_bar = Some(shown);
            window.set_native_titlebar_visibility(shown);
        }
    }

    /// Shows the controls and restarts the "Hide after" delay. Returns whether they were
    /// hidden.
    fn wake_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.situation(window, cx).idles() {
            self.auto_hide.timer.cancel();
            return false;
        }
        let was_idle = self.auto_hide.timer.is_idle();
        let delay = settings::get(cx).prefs.interface.hide_after();
        if let Some(sleep) = self.auto_hide.timer.wake(Instant::now(), delay) {
            Self::run_idle_timer(sleep, window, cx);
        }
        was_idle
    }

    /// The I key: shows the controls, in fullscreen even with "Show controls in fullscreen" off.
    pub(super) fn summon_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_hide.summoned = true;
        self.wake_controls(window, cx);
        cx.notify();
    }

    /// The one timer task: sleeps toward the deadline, which wakes keep moving, then hides.
    fn run_idle_timer(first: Duration, window: &mut Window, cx: &mut Context<Self>) {
        cx.spawn_in(window, async move |this, cx| {
            let mut sleep = first;
            loop {
                cx.background_executor().timer(sleep).await;
                match this.update_in(cx, Self::tick_idle) {
                    Ok(Tick::Sleep(more)) => sleep = more,
                    // Done, or the window is gone.
                    Ok(Tick::Hide | Tick::Stop) | Err(_) => break,
                }
            }
        })
        .detach();
    }

    fn tick_idle(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Tick {
        let held = self.controls_held(window, cx);
        let tick = self.auto_hide.timer.tick(Instant::now(), held);
        if tick == Tick::Hide {
            self.on_rested(window, cx);
        }
        tick
    }

    /// The pointer rested: the controls hide, and the cursor with them when it's over the
    /// window. A control clicked earlier gives the keyboard back to the player, so keys don't
    /// go to a hidden button.
    fn on_rested(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_hide.summoned = false;
        if self.focus_handle.contains_focused(window, cx) && !self.focus_handle.is_focused(window) {
            self.focus_handle.focus(window, cx);
        }
        if window.is_window_active() && self.auto_hide.pointer.is_some() {
            cursor::hide_until_moved();
        }
        cx.notify();
    }

    /// Paints nothing; hears every pointer move over the window, over the bars and popovers
    /// too (an element's own move listener only hears moves over its uncovered parts).
    pub(super) fn pointer_watcher(cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        canvas(|_, _, _| {}, move |_, (), window, _| Self::follow_pointer(this, window))
            .absolute()
            .size_full()
    }

    /// Sends this frame's pointer moves to `this`.
    fn follow_pointer(this: Entity<Self>, window: &mut Window) {
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                this.update(cx, |this, cx| this.on_pointer_moved(event.position, window, cx));
            }
        });
    }

    fn on_pointer_moved(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Moves elsewhere (another screen) leave the controls alone.
        if !Bounds::new(point(Pixels::ZERO, Pixels::ZERO), window.viewport_size())
            .contains(&position)
        {
            self.auto_hide.pointer = None;
            self.auto_hide.anchor.clear();
            return;
        }
        self.auto_hide.pointer = Some(position);
        let moved = self.auto_hide.anchor.moved_to(f32::from(position.x), f32::from(position.y));
        if moved && self.wake_controls(window, cx) {
            cx.notify();
        }
    }

    /// Whether something keeps the controls up (S2.8.3).
    fn controls_held(&self, window: &Window, cx: &App) -> bool {
        self.playback_holds(cx)
            || self.pointer_over_bars(window)
            || self.has_open_overlay()
            || self.seek_bar.is_scrubbing()
            || self.volume_slider.is_dragging()
            || self.keyboard_in_controls(window, cx)
    }

    /// Playback isn't running: stopped, opening, or paused (unless the setting lets paused
    /// video hide them).
    fn playback_holds(&self, cx: &App) -> bool {
        let Some(player) = self.loaded_player(cx) else { return true };
        match player.read(cx).state() {
            PlaybackState::Playing | PlaybackState::Buffering => false,
            PlaybackState::Paused => settings::get(cx).prefs.interface.keep_controls_while_paused,
            _ => true,
        }
    }

    pub(super) fn pointer_over_bars(&self, window: &Window) -> bool {
        let Some(pointer) = self.auto_hide.pointer else { return false };
        let title = (!window.is_fullscreen()).then_some(TITLE_BAR_HEIGHT);
        let height = f32::from(window.viewport_size().height);
        rules::over_bars(f32::from(pointer.y), height, CONTROL_BAR_HEIGHT, title)
    }

    pub(super) fn has_open_overlay(&self) -> bool {
        self.menu.is_some()
            || self.auto_hide.video_menu.is_some()
            || self.control_bar.is_overflow_open()
            || self.track_popover.is_open()
            || self.settings_panel.is_open()
            || self.control_bar.is_editing_time()
    }

    /// Keyboard focus is on one of the window's controls (Tab reached it), not the player.
    fn keyboard_in_controls(&self, window: &Window, cx: &App) -> bool {
        window.last_input_was_keyboard()
            && self.focus_handle.contains_focused(window, cx)
            && !self.focus_handle.is_focused(window)
    }
}
