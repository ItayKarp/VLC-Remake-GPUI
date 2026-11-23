//! The [`Player`]'s picture adjustments (S7.1): hue, brightness, contrast, saturation and gamma;
//! and its video filters (S7.2): deinterlacing, flip and rotation. Both are seen at once and kept
//! for the media after.

use super::Player;
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

    /// Passes video through `filters` from now on; the media playing reopens where it is.
    pub fn set_video_filters(&mut self, filters: VideoFilters) {
        if self.video_filters != filters {
            self.video_filters = filters;
            self.with_backend(|backend| backend.set_video_filters(filters));
        }
    }
}
