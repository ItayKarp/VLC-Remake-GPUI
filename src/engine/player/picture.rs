//! The [`Player`]'s picture adjustments (S7.1): hue, brightness, contrast, saturation and gamma;
//! its video filters (S7.2): deinterlacing, flip and rotation; and its color filters (S7.4). All
//! are seen at once and kept for the media after. And how HDR video is shown (S7.5).

use super::Player;
use crate::engine::colors::Colors;
use crate::engine::hdr::{HdrMode, HdrStatus};
use crate::engine::media_options::{Picture, VideoFilters};

impl Player {
    /// Adjusts the picture as `picture` says from now on, or shows it as the media has it.
    pub fn set_picture(&mut self, picture: Option<Picture>) {
        if self.picture != picture {
            self.picture = picture;
            self.with_backend(|backend| backend.set_picture(picture));
        }
    }

    /// The picture's adjustments in use; `None` while off.
    pub fn picture(&self) -> Option<Picture> {
        self.picture
    }

    /// Shows video through the color filters `colors` turns on from now on.
    pub fn set_colors(&mut self, colors: Colors) {
        if self.colors != colors {
            self.colors = colors;
            self.with_backend(|backend| backend.set_colors(colors));
        }
    }

    /// Shows HDR video as `mode` says from now on; the media playing may reopen where it is.
    pub fn set_hdr_mode(&mut self, mode: HdrMode) {
        if self.hdr_mode != mode {
            self.hdr_mode = mode;
            self.with_backend(|backend| backend.set_hdr_mode(mode));
        }
    }

    /// What the video playing is, by its colors, and how it is tone-mapped.
    pub fn hdr(&self) -> HdrStatus {
        self.info.hdr
    }

    /// Passes video through `filters` from now on; the media playing reopens where it is.
    pub fn set_video_filters(&mut self, filters: VideoFilters) {
        if self.video_filters != filters {
            self.video_filters = filters;
            self.with_backend(|backend| backend.set_video_filters(filters));
        }
    }
}
