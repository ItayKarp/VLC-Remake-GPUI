//! The [`Player`]'s fade (S8.13.1): the sleep timer turns the sound down over its last seconds
//! without moving the volume the user set, so the volume control never shows the fade and the
//! next playback is as loud as before.

use gpui::Context;

use super::Player;
use crate::engine::values::Volume;

/// How much of the set volume is heard: 1.0 is all of it, 0.0 silence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Fade(f32);

impl Default for Fade {
    /// No fade: the volume as set.
    fn default() -> Self {
        Self(1.0)
    }
}

impl Player {
    /// Plays at `loudness` of the volume set, 0.0 (silent) to 1.0 (as set); out-of-range
    /// values are clamped and NaN counts as 1.0. The volume the user set doesn't change.
    pub fn set_fade(&mut self, loudness: f32, cx: &mut Context<Self>) {
        let fade = if loudness.is_nan() { Fade::default() } else { Fade(loudness.clamp(0.0, 1.0)) };
        if std::mem::replace(&mut self.controls.fade, fade) != fade {
            self.push_volume();
            cx.notify();
        }
    }

    /// How much of the set volume is heard; see [`Self::set_fade`].
    pub fn fade(&self) -> f32 {
        self.controls.fade.0
    }

    /// Gives the backend the volume set, faded.
    pub(super) fn push_volume(&mut self) {
        let heard = Volume::new(self.controls.volume.get() * self.controls.fade.0);
        self.with_backend(|backend| backend.set_volume(heard));
    }
}
