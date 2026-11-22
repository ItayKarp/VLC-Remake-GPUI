//! The mouse on the video (S2.10.1), following Settings → Controls (its rows come with S5.6): a
//! single click plays or pauses, a double click enters or leaves fullscreen, the wheel changes
//! the volume (`volume_slider.rs`) and scrolling sideways seeks by the very short skip.
//!
//! A double click's first click must not also pause, so while a double click means something
//! the single click waits [`DOUBLE_CLICK_WAIT`] and a second click cancels it. A click that only
//! closes a popover or menu, that activates the window, or that lands on a bar floating over
//! the video does nothing to playback.

use std::time::Duration;

use gpui::{ClickEvent, Context, MouseButton, MouseDownEvent, ScrollEvent, Window};

use super::MainWindow;
use super::seeking::Way;
use super::wheel::{Axis, Turn, WheelSteps};
use crate::settings::{self, ClickAction, DoubleClickAction, HorizontalWheelAction, SkipLength};

/// How long a single click waits for a second one before it acts.
const DOUBLE_CLICK_WAIT: Duration = Duration::from_millis(300);

/// The video's clicks and sideways scrolling.
pub struct VideoMouse {
    /// Whether the press under way may act when it's released.
    armed: bool,
    /// Counts single clicks, so a double click can cancel the one that's waiting.
    clicks: u64,
    sideways: WheelSteps,
}

impl Default for VideoMouse {
    fn default() -> Self {
        Self { armed: false, clicks: 0, sideways: WheelSteps::new(Axis::Horizontal) }
    }
}

impl MainWindow {
    /// Before anything else sees a press on the video (a popover closing on it, say): decides
    /// whether its click may act.
    pub(super) fn arm_video_click(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let over_bars = self.chrome(window, cx).controls && self.pointer_over_bars(window, cx);
        self.video_mouse.armed = event.button == MouseButton::Left
            && !event.first_mouse
            && !over_bars
            && !self.has_open_overlay();
    }

    /// A click on the video.
    pub(super) fn on_video_click(
        &mut self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !std::mem::take(&mut self.video_mouse.armed) || event.is_keyboard() {
            return;
        }
        match event.click_count() {
            1 => self.single_click(window, cx),
            2 => self.double_click(window, cx),
            _ => {}
        }
    }

    fn single_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let controls = &settings::get(cx).prefs.controls;
        if controls.click == ClickAction::Nothing {
            return;
        }
        if controls.double_click == DoubleClickAction::Nothing {
            return self.play_pause(window, cx);
        }
        self.video_mouse.clicks += 1;
        let click = self.video_mouse.clicks;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(DOUBLE_CLICK_WAIT).await;
            // The window may be gone by then; nothing is left to play.
            let _ = this.update_in(cx, |this, window, cx| this.waited_click(click, window, cx));
        })
        .detach();
    }

    /// Plays or pauses for the single click `click`, unless a second click made it a double.
    fn waited_click(&mut self, click: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.video_mouse.clicks == click {
            self.play_pause(window, cx);
        }
    }

    fn double_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The first click's play or pause is off.
        self.video_mouse.clicks += 1;
        if settings::get(cx).prefs.controls.double_click == DoubleClickAction::ToggleFullscreen {
            self.toggle_fullscreen(window, cx);
        }
    }

    /// Scrolling over the video: up and down for the volume, sideways to seek.
    pub(super) fn scroll_on_video(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match Axis::of(event.delta) {
            Axis::Vertical => self.scroll_volume_on_video(event, window, cx),
            Axis::Horizontal => self.scroll_seek(event, cx),
        }
    }

    /// Seeks by the very short skip as the wheel turns sideways: left is back.
    fn scroll_seek(&mut self, event: &ScrollEvent, cx: &mut Context<Self>) {
        if settings::get(cx).prefs.controls.horizontal_wheel == HorizontalWheelAction::Nothing {
            return;
        }
        let way = match self.video_mouse.sideways.take(event.delta, event.touch_phase) {
            Some(Turn::Positive) => Way::Back,
            Some(Turn::Negative) => Way::Forward,
            None => return,
        };
        self.skip(way, SkipLength::VeryShort, cx);
    }
}
