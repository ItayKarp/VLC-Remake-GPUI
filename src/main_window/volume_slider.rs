//! The volume control (S2.5, `volume/volume-*.png`): the speaker, which mutes and shows the
//! level (S2.5.4), and the wedge slider (S2.5.1) on a non-linear scale with a magnet at 100%
//! (S2.5.2, S2.5.6).
//!
//! Drag or click the wedge, scroll over it (or over the video, per Settings → Controls), or
//! use the arrows while it has focus; Up/Down work anywhere. Every change shows the value in a
//! bubble over the wedge (S2.5.3), or on the video while the control bar is hidden (S2.5.7).
//! Changing the volume unmutes, as in VLC; 0% is not muted. Without an audio device the wedge
//! is disabled and its tooltip says why.

mod render;
mod scale;
#[cfg(test)]
mod screenshots;
mod wedge;

use std::time::Duration;

use gpui::{
    Action, App, Context, Entity, FocusHandle, KeyDownEvent, MouseDownEvent, Pixels, Point,
    ScrollEvent, Window,
};

use self::scale::{Scale, WheelSteps};
use super::MainWindow;
use super::flash::Flash;
use crate::engine::player::Player;
use crate::engine::values::{Volume, VolumeChange};
use crate::keymap::actions as act;
use crate::settings::{self, WheelAction};
use crate::ui::popover::BoundsTracker;

/// How long the value bubble stays after a key or the wheel changed the volume.
const BUBBLE_SHOWN_FOR: Duration = Duration::from_millis(1000);

/// The volume slider's own state.
pub struct VolumeSlider {
    track: BoundsTracker,
    focus: FocusHandle,
    dragging: bool,
    wheel: WheelSteps,
    /// The value bubble after a key or the wheel.
    bubble: Flash,
}

impl VolumeSlider {
    /// A volume slider that is a tab stop of `cx`'s window.
    pub fn new(cx: &mut App) -> Self {
        Self {
            track: BoundsTracker::new(),
            focus: cx.focus_handle().tab_stop(true),
            dragging: false,
            wheel: WheelSteps::default(),
            bubble: Flash::default(),
        }
    }

    /// Whether the wedge is being dragged.
    pub(super) fn is_dragging(&self) -> bool {
        self.dragging
    }
}

/// The volume as the bubble and screen readers say it ("100 %", as designed).
fn percent_text(volume: Volume) -> String {
    format!("{} %", volume.as_percent())
}

impl MainWindow {
    /// The loaded player, if its volume can change: it has an output device.
    fn audible(&self, cx: &App) -> Option<Entity<Player>> {
        self.loaded_player(cx).filter(|player| !player.read(cx).has_no_audio_device())
    }

    /// Shows the new volume: in the bubble over the wedge, or on the video while the control
    /// bar is hidden (S2.8).
    pub(super) fn show_volume(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        if self.chrome(window, cx).controls {
            self.flash(|this| &mut this.volume_slider.bubble, BUBBLE_SHOWN_FOR, cx);
        } else {
            let text = format!("Volume {}", percent_text(player.read(cx).volume()));
            self.show_osd(text, cx);
        }
    }

    /// Scrolling over the video changes the volume, unless Settings → Controls says otherwise.
    pub(super) fn scroll_on_video(
        &mut self,
        event: &ScrollEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if settings::get(cx).prefs.controls.wheel == WheelAction::Volume {
            self.scroll_volume(event, window, cx);
        }
    }

    /// Steps the volume as the wheel turns.
    fn scroll_volume(&mut self, event: &ScrollEvent, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(change) = self.volume_slider.wheel.take(event.delta, event.touch_phase) {
            self.step_volume(change, window, cx);
        }
    }

    /// Starts a drag where the wedge was pressed: the volume jumps there.
    fn start_volume_drag(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.volume_slider.focus.focus(window, cx);
        if self.audible(cx).is_none() {
            return;
        }
        self.volume_slider.dragging = true;
        self.drag_volume_to(event.position, cx);
    }

    /// Follows the pointer during a drag, wherever it is.
    fn drag_volume_to(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(player) = self.audible(cx) else { return };
        let Some(bounds) = self.volume_slider.track.bounds() else { return };
        let width = f32::from(bounds.size.width).max(1.);
        let fraction = f32::from(position.x - bounds.left()) / width;
        let volume = Scale::new(player.read(cx).max_volume()).dragged_to(fraction);
        player.update(cx, |player, cx| player.set_volume(volume, cx));
        cx.notify();
    }

    /// Ends a drag when the button is released anywhere.
    fn end_volume_drag(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.volume_slider.dragging) {
            cx.notify();
        }
    }

    /// The arrows step the volume while the wedge has focus (Left and Down quieter), Home
    /// silences it and End goes to 100%, not into the boost.
    fn volume_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keystroke = &event.keystroke;
        if keystroke.modifiers.modified() {
            return;
        }
        let step: &dyn Action = match keystroke.key.as_str() {
            "up" | "right" => &act::VolumeUp,
            "down" | "left" => &act::VolumeDown,
            "home" => return self.set_volume_by_key(Volume::new(0.), window, cx),
            "end" => return self.set_volume_by_key(Volume::FULL, window, cx),
            _ => return,
        };
        cx.stop_propagation();
        // Through the held-key gate, like Up and Down anywhere else.
        self.run_player_command(step, window, cx);
    }

    fn set_volume_by_key(&mut self, volume: Volume, window: &mut Window, cx: &mut Context<Self>) {
        cx.stop_propagation();
        let Some(player) = self.audible(cx) else { return };
        player.update(cx, |player, cx| player.set_volume(volume, cx));
        self.show_volume(window, cx);
    }

    /// Raises or lowers the volume by the "Volume step" and shows it.
    pub(super) fn step_volume(
        &mut self,
        change: VolumeChange,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(player) = self.audible(cx) else { return };
        let step = super::audio::volume_step(settings::get(cx));
        player.update(cx, |player, cx| player.step_volume(change, step, cx));
        self.show_volume(window, cx);
    }
}
