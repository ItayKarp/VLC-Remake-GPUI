//! The video filters on the worker thread (S7.2, S5.8.1): deinterlacing, flip and rotation, set
//! as the player's variables (`core_vars.rs`), which each video output reads as it opens. A
//! change reopens the media playing where it is (`reopen.rs`), so it is seen at once.
//!
//! libVLC 3 transforms only software-decoded pictures, so a transform reopens the media in
//! software. And its filter chain gives the output a picture of the size it started with, so a
//! quarter turn arrives squeezed into the unturned size: the sink asks for the turned size, which
//! libVLC stretches it back to (`vmem.rs`).

use std::ffi::{CStr, CString};
use std::sync::atomic::Ordering;

use super::core_vars::Var;
use super::decoding::Decoding;
use super::reopen::Output;
use super::worker::Worker;
use crate::engine::media_options::{Deinterlace, Deinterlacing, VideoFilters};
use crate::engine::tracks::TrackKind;
use crate::engine::transform::Transform;

/// The player's variables the filters are set through.
const VIDEO_FILTER: &CStr = c"video-filter";
const TRANSFORM_TYPE: &CStr = c"transform-type";
const DEINTERLACE: &CStr = c"deinterlace";
const DEINTERLACE_MODE: &CStr = c"deinterlace-mode";
/// The `transform` filter's name in the `video-filter` chain, which may hold `adjust` too.
const TRANSFORM: &str = "transform";
/// `deinterlace`'s values.
const DEINTERLACE_AUTOMATIC: i64 = -1;
const DEINTERLACE_OFF: i64 = 0;
const DEINTERLACE_ON: i64 = 1;

impl Worker {
    /// Passes video through `filters` from now on, reopening the media playing for it.
    pub(super) fn set_video_filters(&mut self, filters: VideoFilters) {
        if filters == self.video_filters {
            return;
        }
        self.video_filters = filters;
        self.apply_video_filters();
        if self.shows_video() {
            self.reopen_for_filters();
        }
    }

    /// Reopens the media playing where it is with a new video output, which builds the player's
    /// filters into it; in software if they need it.
    pub(super) fn reopen_for_filters(&mut self) {
        if self.filters_need_software() && self.decoding == Decoding::Hardware {
            self.decoding = Decoding::Software;
            super::hardware::decoding(None);
        }
        tracing::info!("libvlc: new video filters; reopening the media where it is");
        self.reopen(self.playing_tracks(), Output::Rebuilt);
    }

    /// Whether the filters only work on software-decoded pictures: libVLC 3 can't transform
    /// VideoToolbox's frames, nor enhance them (S7.3).
    pub(super) fn filters_need_software(&self) -> bool {
        self.video_filters.transform != Transform::NONE || self.enhanced
    }

    /// Sets the player's variables for the filters and the media open now, and tells the sink
    /// whether the picture's sides swap.
    pub(super) fn apply_video_filters(&self) {
        let VideoFilters { transform, deinterlace } = self.video_filters;
        let transform =
            if self.quarter_turned { transform.for_quarter_turned() } else { transform };
        let player = self.handles.player;
        // SAFETY: the player is live.
        let current = unsafe { self.vlc.player_string(player, VIDEO_FILTER) };
        let kind = transform.libvlc_type();
        let chain = filter_chain(current.as_deref(), kind.is_some());
        let (mode, method) = deinterlace_values(deinterlace);
        let texts = [chain.as_str(), kind.unwrap_or_default(), method].map(CString::new);
        let [Ok(chain), Ok(kind), Ok(method)] = texts else { return };
        // SAFETY: the player is live.
        unsafe {
            self.vlc.set_player_var(player, VIDEO_FILTER, Var::String(&chain));
            self.vlc.set_player_var(player, TRANSFORM_TYPE, Var::String(&kind));
            self.vlc.set_player_var(player, DEINTERLACE, Var::Integer(mode));
            self.vlc.set_player_var(player, DEINTERLACE_MODE, Var::String(&method));
        }
        if let Some(video) = &self.video {
            video.set_transposed(transform.swaps_sides());
        }
    }

    /// Whether video from the open media is showing, so new filters need a reopen.
    pub(super) fn shows_video(&self) -> bool {
        !self.media.is_null()
            && self.shared.started.load(Ordering::Relaxed)
            && self.selected(TrackKind::Video).is_some()
    }
}

/// The `video-filter` chain `current` with the `transform` filter in it or not, the others
/// (`adjust`) kept as they are.
fn filter_chain(current: Option<&str>, transform: bool) -> String {
    let others = current.unwrap_or_default().split(':');
    let mut filters: Vec<&str> =
        others.filter(|filter| !filter.is_empty() && *filter != TRANSFORM).collect();
    if transform {
        filters.insert(0, TRANSFORM);
    }
    filters.join(":")
}

/// `deinterlace` and `deinterlace-mode` for `deinterlacing`.
fn deinterlace_values(deinterlacing: Deinterlacing) -> (i64, &'static str) {
    let mode = match deinterlacing.mode {
        Deinterlace::Automatic => DEINTERLACE_AUTOMATIC,
        Deinterlace::On => DEINTERLACE_ON,
        Deinterlace::Off => DEINTERLACE_OFF,
    };
    (mode, deinterlacing.method.libvlc_name())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::media_options::DeinterlaceMethod;

    #[test]
    fn the_transform_joins_the_chain_and_leaves_it_alone() {
        assert_eq!(filter_chain(None, true), "transform");
        assert_eq!(filter_chain(Some("adjust"), true), "transform:adjust");
        assert_eq!(filter_chain(Some("transform:adjust"), false), "adjust");
        assert_eq!(filter_chain(Some("transform"), true), "transform");
        assert_eq!(filter_chain(Some(""), false), "");
    }

    #[test]
    fn deinterlacing_becomes_libvlcs_values() {
        let on = Deinterlacing { mode: Deinterlace::On, method: DeinterlaceMethod::Yadif2x };
        assert_eq!(deinterlace_values(on), (DEINTERLACE_ON, "yadif2x"));
        assert_eq!(deinterlace_values(Deinterlacing::default()), (DEINTERLACE_AUTOMATIC, "auto"));
    }
}
