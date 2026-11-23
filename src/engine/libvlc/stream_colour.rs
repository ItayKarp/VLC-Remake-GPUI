//! A video's colors as libVLC's core describes them (S7.5).
//!
//! libVLC 3's API tells neither a video's transfer function nor its HDR metadata, but the input
//! playing lists them for each stream in its item's description, the media information VLC
//! shows ("Color transfer function", "MaxCLL"), once the decoder has reported the picture's
//! format: a moment after the video starts, not before.
//!
//! The names are VLC's English ones; the libVLC shipped here has no translations. A translated
//! libVLC (a Linux system package) names them otherwise, and its video then counts as SDR,
//! though HDR10 and HLG can still be chosen.

use std::ffi::{CStr, CString, c_void};

use super::api::{LibVlc, MediaPlayer};
use super::core_vars::INPUT;
use crate::engine::hdr::{SourceColour, Transfer};

/// The most streams looked through for the video.
const MAX_STREAMS: usize = 32;
/// The lines of a stream's description read here.
const TYPE: &CStr = c"Type";
const TRANSFER: &CStr = c"Color transfer function";
const MAX_CLL: &CStr = c"MaxCLL";
const MAX_LUMINANCE: &CStr = c"Max. luminance";
const CODEC: &CStr = c"Codec";
/// A video stream's type, and the HDR transfer functions' names.
const VIDEO: &str = "Video";
const PQ: &str = "SMPTE ST2084 (PQ)";
const HLG: &str = "Hybrid Log-Gamma";
/// What a Dolby Vision stream's codec line holds: its description or one of its FourCCs.
const DOLBY_VISION: [&str; 5] = ["Dolby Vision", "(dvh1)", "(dvhe)", "(dva1)", "(dvav)"];

/// The lines of a video stream's description that say what its colors are.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ColourLines {
    pub(super) transfer: Option<String>,
    pub(super) max_cll: Option<String>,
    pub(super) max_luminance: Option<String>,
    pub(super) codec: Option<String>,
}

impl ColourLines {
    /// Whether the decoder has said what the transfer function is.
    pub(super) fn knows_transfer(&self) -> bool {
        self.transfer.is_some()
    }

    /// The colors these lines describe.
    pub(super) fn colour(&self) -> SourceColour {
        let transfer = match self.transfer.as_deref() {
            Some(PQ) => Some(Transfer::Pq),
            Some(HLG) => Some(Transfer::Hlg),
            _ => None,
        };
        let peak = [&self.max_cll, &self.max_luminance]
            .into_iter()
            .find_map(|line| line.as_deref().and_then(nits).filter(|nits| *nits > 0.));
        let codec = self.codec.as_deref().unwrap_or_default();
        let dolby_vision = DOLBY_VISION.iter().any(|name| codec.contains(name));
        SourceColour { transfer, peak, dolby_vision }
    }
}

impl LibVlc {
    /// The lines describing the colors of the first video stream of the input `player` plays;
    /// `None` while it plays none.
    ///
    /// # Safety
    ///
    /// `player` is a live `libvlc_media_player_t`.
    pub(super) unsafe fn video_colour(&self, player: *mut MediaPlayer) -> Option<ColourLines> {
        let mut found = None;
        // SAFETY: the caller's promise; the input is held during the call, and with it its item.
        unsafe {
            self.for_each_child(player, INPUT, |input| {
                let item = (self.core.input_item)(input);
                if found.is_none() && !item.is_null() {
                    found = self.video_lines(item);
                }
            });
        }
        found
    }

    /// The colour lines of the first video stream `item` describes.
    ///
    /// # Safety
    ///
    /// `item` is a live `input_item_t`.
    unsafe fn video_lines(&self, item: *mut c_void) -> Option<ColourLines> {
        (0..MAX_STREAMS).find_map(|stream| {
            let category = CString::new(format!("Stream {stream}")).ok()?;
            // SAFETY: the caller's promise.
            let line = |name: &CStr| unsafe { self.info(item, &category, name) };
            (line(TYPE).as_deref() == Some(VIDEO)).then(|| ColourLines {
                transfer: line(TRANSFER),
                max_cll: line(MAX_CLL),
                max_luminance: line(MAX_LUMINANCE),
                codec: line(CODEC),
            })
        })
    }

    /// The line `name` of `category` in `item`'s description; `None` when it has none.
    ///
    /// # Safety
    ///
    /// `item` is a live `input_item_t`.
    unsafe fn info(&self, item: *mut c_void, category: &CStr, name: &CStr) -> Option<String> {
        // SAFETY: the caller's promise; the core hands over a copy, which `take_string` frees.
        unsafe {
            let text = (self.core.item_info)(item, category.as_ptr(), name.as_ptr());
            self.take_string(text)
        }
    }
}

/// The brightness in "1000 cd/m²" or "1000.0000 cd/m²".
fn nits(line: &str) -> Option<f32> {
    line.split_whitespace().next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(transfer: &str, max_cll: &str, codec: &str) -> ColourLines {
        let line = |text: &str| (!text.is_empty()).then(|| text.to_owned());
        ColourLines {
            transfer: line(transfer),
            max_cll: line(max_cll),
            max_luminance: Some("4000.0000 cd/m²".to_owned()),
            codec: line(codec),
        }
    }

    #[test]
    fn hdr10_reads_its_transfer_and_peak() {
        let colour = lines(PQ, "1000 cd/m²", "MPEG-H Part2/HEVC (H.265) (hevc)").colour();
        let expected = SourceColour { transfer: Some(Transfer::Pq), peak: Some(1000.), ..colour };
        assert_eq!(colour, expected);
        assert!(!colour.dolby_vision);
    }

    #[test]
    fn a_missing_or_zero_maxcll_falls_back_to_the_mastering_peak() {
        assert_eq!(lines(PQ, "", "").colour().peak, Some(4000.));
        assert_eq!(lines(PQ, "0 cd/m²", "").colour().peak, Some(4000.));
    }

    #[test]
    fn sdr_and_hlg_and_dolby_vision() {
        assert_eq!(lines("ITU-R BT.709", "", "").colour().transfer, None);
        assert_eq!(lines(HLG, "", "").colour().transfer, Some(Transfer::Hlg));
        let dolby = lines("", "", "Dolby Vision HEVC (H.265) (dvh1)");
        assert!(!dolby.knows_transfer());
        assert!(dolby.colour().is_unshowable_dolby_vision());
    }
}
