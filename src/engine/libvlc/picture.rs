//! The picture adjustments on the worker thread (S7.1, S5.8.1): libVLC's `adjust` filter set
//! through `libvlc_video_set_adjust_*`, which sets each value on the player, for the video
//! outputs it makes later, and on the outputs already showing, so a change is seen at once.
//!
//! Turning the filter on or off rebuilds the output's filter chain, a brief re-init; moving a
//! value while it is on doesn't, which is what lets a slider drag apply live.

use std::ffi::{c_float, c_int, c_uint};

use super::worker::Worker;
use crate::engine::media_options::Picture;

/// libVLC's `libvlc_video_adjust_option_t`.
const ENABLE: c_uint = 0;
const CONTRAST: c_uint = 1;
const BRIGHTNESS: c_uint = 2;
const HUE: c_uint = 3;
const SATURATION: c_uint = 4;
const GAMMA: c_uint = 5;

impl Worker {
    /// Shows the picture adjusted as `picture` says from now on, or as the media has it.
    pub(super) fn set_picture(&self, picture: Option<Picture>) {
        let api = &self.vlc.api;
        let player = self.handles.player;
        // SAFETY: the player is live; these only set its variables and its outputs'.
        unsafe {
            if let Some(picture) = picture {
                for (option, value) in values(&picture) {
                    (api.libvlc_video_set_adjust_float)(player, option, value);
                }
            }
            (api.libvlc_video_set_adjust_int)(player, ENABLE, c_int::from(picture.is_some()));
        }
    }
}

/// Each of `picture`'s values by libVLC's option for it.
fn values(picture: &Picture) -> [(c_uint, c_float); 5] {
    [
        (CONTRAST, picture.contrast),
        (BRIGHTNESS, picture.brightness),
        (HUE, picture.hue),
        (SATURATION, picture.saturation),
        (GAMMA, picture.gamma),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_value_goes_to_its_option() {
        let picture = Picture { hue: -30., gamma: 2., ..Picture::NEUTRAL };
        let values = values(&picture);
        assert!(values.contains(&(HUE, -30.)));
        assert!(values.contains(&(GAMMA, 2.)));
        assert!(values.contains(&(CONTRAST, 1.)));
    }
}
