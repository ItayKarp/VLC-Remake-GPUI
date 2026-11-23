//! The filters that work on decoded pictures, on the worker thread: the picture's enhancements
//! (S7.3, `enhance.rs`) and the color filters (S7.4, `color_filters.rs`), each in the
//! `video-filter` chain while it is on.
//!
//! As libVLC's own `libvlc_video_set_adjust_*` do, each filter's settings are set on the player,
//! for the video outputs it makes later, and on the outputs running now, whose filters take them
//! at once; and a filter turned on or off is written into the chain of both, which rebuilds a
//! running output's filters, a brief re-init. So a slider drag applies live, and only turning a
//! filter on or off re-inits.
//!
//! The chain has one fixed order ([`ORDER`]), whatever order the filters were turned on in: the
//! enhancements that repair the picture, then the color filters in the panel's order, then film
//! grain, which lies over everything as on film. The filters the player sets otherwise
//! (`transform`, `adjust`) keep their places before them.
//!
//! None of them work on hardware-decoded pictures, so turning one on over VideoToolbox's video
//! reopens the media where it is, in software (`video_filters.rs`), and later media opens in
//! software while one is on.

use std::ffi::{CStr, CString};

use super::core_vars::Var;
use super::decoding::Decoding;
use super::worker::Worker;

/// The player's chain of video filters.
const VIDEO_FILTER: &CStr = c"video-filter";
/// The filters set here, in the order video passes through them.
const ORDER: [&str; 9] = [
    "gradfun",
    "sharpen",
    "extract",
    "colorthres",
    "invert",
    "posterize",
    "gradient",
    "sepia",
    "grain",
];

/// One of a filter's variables and its value.
pub(super) type Setting = (&'static CStr, Var<'static>);

/// A filter that is on: its name in the chain, and its settings.
#[derive(Clone, Debug)]
pub(super) struct OnFilter {
    pub(super) name: &'static str,
    pub(super) settings: Vec<Setting>,
}

/// The groups of filters, each set as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Group {
    /// Sharpening, banding removal and film grain.
    Enhancements,
    /// The color filters.
    Colors,
}

/// The filters on, by group.
#[derive(Debug, Default)]
pub(super) struct CpuFilters {
    enhancements: Vec<OnFilter>,
    colors: Vec<OnFilter>,
}

impl CpuFilters {
    /// Whether any filter is on, which needs software decoding.
    pub(super) fn any(&self) -> bool {
        !self.enhancements.is_empty() || !self.colors.is_empty()
    }

    fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.enhancements.iter().chain(&self.colors).map(|filter| filter.name)
    }
}

impl Worker {
    /// Passes video through exactly the filters `on` of `group` from now on, with their
    /// settings; the other group's are left as they are.
    pub(super) fn set_cpu_filters(&mut self, group: Group, on: Vec<OnFilter>) {
        let player = self.handles.player;
        for (variable, value) in on.iter().flat_map(|filter| &filter.settings) {
            // SAFETY: the player is live.
            unsafe {
                self.vlc.set_player_var(player, variable, *value);
                self.vlc.set_output_var(player, variable, *value);
            }
        }
        match group {
            Group::Enhancements => self.cpu_filters.enhancements = on,
            Group::Colors => self.cpu_filters.colors = on,
        }
        // SAFETY: the player is live.
        let current = unsafe { self.vlc.player_string(player, VIDEO_FILTER) }.unwrap_or_default();
        let chain = chain(&current, self.cpu_filters.names());
        if chain == current {
            return;
        }
        let Ok(chain) = CString::new(chain) else { return };
        // SAFETY: the player is live.
        unsafe { self.vlc.set_player_var(player, VIDEO_FILTER, Var::String(&chain)) };
        if self.cpu_filters.any() && self.decoding == Decoding::Hardware && self.shows_video() {
            self.reopen_for_filters();
        } else {
            // SAFETY: the player is live.
            unsafe { self.vlc.set_output_var(player, VIDEO_FILTER, Var::String(&chain)) };
        }
    }
}

/// The `video-filter` chain `current` with exactly the filters `on` at its end in [`ORDER`], the
/// others (`transform`, `adjust`) kept as they are.
fn chain(current: &str, on: impl Iterator<Item = &'static str>) -> String {
    let others = current.split(':').filter(|filter| !filter.is_empty() && !ORDER.contains(filter));
    let mut on: Vec<&str> = on.collect();
    on.sort_by_key(|name| ORDER.iter().position(|filter| filter == name));
    others.chain(on).collect::<Vec<_>>().join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_filters_join_the_chain_after_the_others_in_order() {
        assert_eq!(chain("", ["sharpen"].into_iter()), "sharpen");
        assert_eq!(chain("transform:adjust", ["grain"].into_iter()), "transform:adjust:grain");
        assert_eq!(chain("sharpen:adjust:grain", ["gradfun"].into_iter()), "adjust:gradfun");
        assert_eq!(chain("transform:sharpen", std::iter::empty()), "transform");
    }

    #[test]
    fn the_order_is_fixed_whatever_order_they_came_in() {
        let on = ["grain", "sepia", "invert", "extract", "sharpen"];
        assert_eq!(chain("adjust", on.into_iter()), "adjust:sharpen:extract:invert:sepia:grain");
    }
}
