//! The color filters (Settings → Video → Colors, S7.4): Color Extraction, Color Threshold, Negate
//! Colors, Posterize, Gradient and Sepia, all off by default, as in VLC.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;
use crate::engine::colors::{ColorThreshold, Colors, Gradient, GradientMode};
use crate::settings::registry::{format_hex_color, hex_color};

/// The color the two color pickers start with, and what a color that can't be read becomes.
const RED: u32 = 0xFF_0000;
/// libVLC's defaults for Color Threshold's saturation and similarity, and Posterize's levels.
const THRESHOLD_SATURATION: u32 = 20;
const THRESHOLD_SIMILARITY: u32 = 15;
const POSTERIZE_LEVELS: u32 = 6;

/// "Colors", in libVLC's units; the registry keeps each in range.
#[allow(
    clippy::struct_excessive_bools,
    reason = "each filter's own switch, a row of its own in the panel and in Settings → Video"
)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorSettings {
    /// "Use color filters": off, the filters below are kept but none is used.
    #[serde(deserialize_with = "lenient")]
    pub enabled: bool,
    /// "Color Extraction".
    #[serde(deserialize_with = "lenient")]
    pub extract: bool,
    /// The color it keeps, `#RRGGBB`.
    #[serde(deserialize_with = "lenient")]
    pub extract_color: String,
    /// "Color Threshold".
    #[serde(deserialize_with = "lenient")]
    pub threshold: bool,
    /// The color it keeps, `#RRGGBB`.
    #[serde(deserialize_with = "lenient")]
    pub threshold_color: String,
    /// Its "Saturation", 0 to 255.
    #[serde(deserialize_with = "lenient")]
    pub threshold_saturation: u32,
    /// Its "Similarity", 0 to 255.
    #[serde(deserialize_with = "lenient")]
    pub threshold_similarity: u32,
    /// "Negate Colors".
    #[serde(deserialize_with = "lenient")]
    pub negate: bool,
    /// "Posterize".
    #[serde(deserialize_with = "lenient")]
    pub posterize: bool,
    /// Its "Levels" per channel, 2 to 256.
    #[serde(deserialize_with = "lenient")]
    pub posterize_levels: u32,
    /// "Gradient".
    #[serde(deserialize_with = "lenient")]
    pub gradient: bool,
    /// Its mode.
    #[serde(deserialize_with = "lenient")]
    pub gradient_mode: GradientMode,
    /// Its "Color": keeps the picture's colors.
    #[serde(deserialize_with = "lenient")]
    pub gradient_color: bool,
    /// Its "Cartoon".
    #[serde(deserialize_with = "lenient")]
    pub cartoon: bool,
    /// "Sepia Intensity", 0 to 255; 0 is off.
    #[serde(deserialize_with = "lenient")]
    pub sepia: u32,
}

impl ColorSettings {
    /// The filters to show video through: none while off.
    pub fn applied(&self) -> Colors {
        if !self.enabled {
            return Colors::NONE;
        }
        let threshold = ColorThreshold {
            color: color(&self.threshold_color),
            saturation: self.threshold_saturation,
            similarity: self.threshold_similarity,
        };
        let gradient = Gradient {
            mode: self.gradient_mode,
            colored: self.gradient_color,
            cartoon: self.cartoon,
        };
        Colors {
            extract: self.extract.then(|| color(&self.extract_color)),
            threshold: self.threshold.then_some(threshold),
            negate: self.negate,
            posterize: self.posterize.then_some(self.posterize_levels),
            gradient: self.gradient.then_some(gradient),
            sepia: (self.sepia > 0).then_some(self.sepia),
        }
    }
}

impl Default for ColorSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            extract: false,
            extract_color: format_hex_color(RED),
            threshold: false,
            threshold_color: format_hex_color(RED),
            threshold_saturation: THRESHOLD_SATURATION,
            threshold_similarity: THRESHOLD_SIMILARITY,
            negate: false,
            posterize: false,
            posterize_levels: POSTERIZE_LEVELS,
            gradient: false,
            gradient_mode: GradientMode::default(),
            gradient_color: false,
            cartoon: false,
            sepia: 0,
        }
    }
}

/// A color field's color; red if it can't be read (a hand-edited file).
fn color(text: &str) -> u32 {
    hex_color(text).unwrap_or(RED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_applies_while_off_or_by_default() {
        assert!(ColorSettings::default().applied().is_none());
        let off = ColorSettings { negate: true, ..ColorSettings::default() };
        assert!(off.applied().is_none(), "the section is off");
        let on = ColorSettings { enabled: true, ..ColorSettings::default() };
        assert!(on.applied().is_none(), "no filter is on");
    }

    #[test]
    fn each_filter_applies_with_its_values() {
        let colors = ColorSettings {
            enabled: true,
            extract: true,
            extract_color: "#00ff00".to_owned(),
            posterize: true,
            sepia: 120,
            ..ColorSettings::default()
        }
        .applied();
        assert_eq!(colors.extract, Some(0x00_FF00));
        assert_eq!(colors.posterize, Some(POSTERIZE_LEVELS));
        assert_eq!(colors.sepia, Some(120));
        assert_eq!(colors.threshold, None);
    }

    #[test]
    fn an_unreadable_color_is_red() {
        assert_eq!(color("not a color"), RED);
    }
}
