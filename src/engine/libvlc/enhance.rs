//! The picture's enhancements on the worker thread (S7.3): libVLC's `gradfun` (banding removal),
//! `sharpen` and `grain` filters, each in the `video-filter` chain while its value isn't 0.
//!
//! As libVLC's own `libvlc_video_set_adjust_*` do, each value is set on the player, for the video
//! outputs it makes later, and on the outputs running now, whose filters take it at once; and a
//! filter turned on or off is written into the chain of both, which rebuilds a running output's
//! filters, a brief re-init. So a slider drag applies live, and only crossing 0 re-inits.
//!
//! The filters only work on software-decoded pictures, so turning one on over hardware-decoded
//! video reopens the media where it is, in software (`video_filters.rs`), and later media opens
//! in software while one is on.

use std::ffi::{CStr, CString};

use super::core_vars::Var;
use super::decoding::Decoding;
use super::worker::Worker;
use crate::engine::media_options::Picture;

/// The player's chain of video filters.
const VIDEO_FILTER: &CStr = c"video-filter";

/// One enhancement filter: its name in the chain and the variable holding its value.
struct Filter {
    name: &'static str,
    variable: &'static CStr,
}

/// Banding removal: smooths gradients over this many pixels.
const GRADFUN: Filter = Filter { name: "gradfun", variable: c"gradfun-radius" };
/// Sharpening.
const SHARPEN: Filter = Filter { name: "sharpen", variable: c"sharpen-sigma" };
/// Film grain, added last so the sharpening doesn't sharpen it.
const GRAIN: Filter = Filter { name: "grain", variable: c"grain-variance" };

impl Worker {
    /// Passes video through the enhancements `picture` turns on from now on; `None` turns them
    /// all off.
    pub(super) fn set_enhancements(&mut self, picture: Option<&Picture>) {
        let player = self.handles.player;
        let values = values(picture);
        for (filter, value) in &values {
            if let Some(value) = *value {
                // SAFETY: the player is live.
                unsafe {
                    self.vlc.set_player_var(player, filter.variable, value);
                    self.vlc.set_output_var(player, filter.variable, value);
                }
            }
        }
        let on = values.iter().filter(|(_, value)| value.is_some()).map(|(filter, _)| filter.name);
        // SAFETY: the player is live.
        let current = unsafe { self.vlc.player_string(player, VIDEO_FILTER) }.unwrap_or_default();
        let chain = enhanced_chain(&current, on);
        self.enhanced = values.iter().any(|(_, value)| value.is_some());
        if chain == current {
            return;
        }
        let Ok(chain) = CString::new(chain) else { return };
        // SAFETY: the player is live.
        unsafe { self.vlc.set_player_var(player, VIDEO_FILTER, Var::String(&chain)) };
        if self.enhanced && self.decoding == Decoding::Hardware && self.shows_video() {
            self.reopen_for_filters();
        } else {
            // SAFETY: the player is live.
            unsafe { self.vlc.set_output_var(player, VIDEO_FILTER, Var::String(&chain)) };
        }
    }
}

/// Each enhancement filter with its value in `picture`, `None` while it is off.
fn values(picture: Option<&Picture>) -> [(Filter, Option<Var<'static>>); 3] {
    let picture = picture.copied().unwrap_or(Picture::NEUTRAL);
    let radius = (picture.deband_radius > 0).then(|| Var::Integer(picture.deband_radius.into()));
    let float = |value: f32| (value > 0.).then_some(Var::Float(value));
    [(GRADFUN, radius), (SHARPEN, float(picture.sharpen)), (GRAIN, float(picture.grain))]
}

/// The `video-filter` chain `current` with exactly the enhancement filters `on` at its end, in
/// order, the others (`transform`, `adjust`) kept as they are.
fn enhanced_chain<'a>(current: &'a str, on: impl Iterator<Item = &'a str>) -> String {
    let enhancement = |filter: &str| [GRADFUN.name, SHARPEN.name, GRAIN.name].contains(&filter);
    let others = current.split(':').filter(|filter| !filter.is_empty() && !enhancement(filter));
    others.chain(on).collect::<Vec<_>>().join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_enhancements_join_the_chain_after_the_others() {
        assert_eq!(enhanced_chain("", ["sharpen"].into_iter()), "sharpen");
        assert_eq!(
            enhanced_chain("transform:adjust", ["grain"].into_iter()),
            "transform:adjust:grain"
        );
        assert_eq!(
            enhanced_chain("sharpen:adjust:grain", ["gradfun"].into_iter()),
            "adjust:gradfun"
        );
        assert_eq!(enhanced_chain("transform:sharpen", std::iter::empty()), "transform");
    }

    #[test]
    fn a_filter_is_on_while_its_value_isnt_zero() {
        let picture = Picture { sharpen: 0.5, deband_radius: 16, ..Picture::NEUTRAL };
        let on: Vec<_> = values(Some(&picture))
            .into_iter()
            .filter(|(_, value)| value.is_some())
            .map(|(filter, _)| filter.name)
            .collect();
        assert_eq!(on, ["gradfun", "sharpen"]);
        assert!(values(None).iter().all(|(_, value)| value.is_none()));
    }
}
