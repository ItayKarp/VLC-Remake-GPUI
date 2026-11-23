//! The shapes Settings → Video and the player's panel stretch and crop the picture to (S5.8.1,
//! S7.2.3).

use serde::{Deserialize, Serialize};

/// "Aspect ratio": the shape the whole picture is stretched to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AspectRatio {
    /// The media's own.
    #[default]
    Automatic,
    /// 1:1.
    Square,
    /// 4:3.
    Standard,
    /// 5:4.
    FiveByFour,
    /// 16:9.
    Wide,
    /// 16:10.
    SixteenByTen,
    /// 2.21:1 (70 mm).
    Ratio221,
    /// 21:9 (ultrawide screens).
    Ultrawide,
    /// 2.35:1 (CinemaScope).
    Ratio235,
    /// 2.39:1 (anamorphic widescreen).
    Ratio239,
    /// The user's own, typed as "Custom ratio" ([`parse_ratio`]).
    Custom,
}

impl AspectRatio {
    /// The ratio as width and height; `None` keeps the media's own, and Custom's is the user's
    /// `custom` one (none while it isn't one).
    pub fn ratio(self, custom: &str) -> Option<(u32, u32)> {
        match self {
            Self::Automatic => None,
            Self::Custom => parse_ratio(custom),
            Self::Square => Some((1, 1)),
            Self::Standard => Some((4, 3)),
            Self::FiveByFour => Some((5, 4)),
            Self::Wide => Some((16, 9)),
            Self::SixteenByTen => Some((16, 10)),
            Self::Ratio221 => Some((221, 100)),
            Self::Ultrawide => Some((21, 9)),
            Self::Ratio235 => Some((235, 100)),
            Self::Ratio239 => Some((239, 100)),
        }
    }
}

/// "Crop": the ratio the picture is trimmed to, keeping its middle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Crop {
    /// The whole picture.
    #[default]
    None,
    /// 1:1.
    Square,
    /// 4:3.
    Standard,
    /// 16:9.
    Wide,
    /// 16:10.
    SixteenByTen,
    /// 1.85:1.
    Ratio185,
    /// 2.21:1.
    Ratio221,
    /// 2.35:1.
    Ratio235,
    /// 2.39:1.
    Ratio239,
}

impl Crop {
    /// The ratio as width and height; `None` keeps the whole picture.
    pub fn ratio(self) -> Option<(u32, u32)> {
        match self {
            Self::None => None,
            Self::Square => Some((1, 1)),
            Self::Standard => Some((4, 3)),
            Self::Wide => Some((16, 9)),
            Self::SixteenByTen => Some((16, 10)),
            Self::Ratio185 => Some((185, 100)),
            Self::Ratio221 => Some((221, 100)),
            Self::Ratio235 => Some((235, 100)),
            Self::Ratio239 => Some((239, 100)),
        }
    }
}

/// What a typed ratio looks like, for a field that refused one.
pub const RATIO_HELP: &str = "A width and height such as 21:9 or 2.35:1, between 1:10 and 10:1";

/// The decimals a typed ratio keeps: "2.35" counts in hundredths.
const RATIO_SCALE: f64 = 1000.;
/// The flattest and tallest shapes a typed ratio may have.
const RATIO_LIMITS: (f64, f64) = (0.1, 10.);

/// A typed aspect ratio as width and height: "21:9", "2.35:1", "16/10", "4 x 3", or one number
/// ("2.39") over 1. `None` when it isn't one, or is flatter or taller than ten to one.
pub fn parse_ratio(text: &str) -> Option<(u32, u32)> {
    let text = text.trim().to_lowercase();
    let (width, height) = [':', '/', 'x']
        .iter()
        .find_map(|separator| text.split_once(*separator))
        .unwrap_or((&text, "1"));
    let (width, height): (f64, f64) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    let shape = width / height;
    if !shape.is_finite() || shape < RATIO_LIMITS.0 || shape > RATIO_LIMITS.1 {
        return None;
    }
    let scaled = |side: f64| (side * RATIO_SCALE).round() as u32;
    Some((scaled(width), scaled(height))).filter(|(width, height)| *width > 0 && *height > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratios_read_in_the_usual_ways() {
        assert_eq!(parse_ratio("21:9"), Some((21_000, 9_000)));
        assert_eq!(parse_ratio(" 2.35 : 1 "), Some((2_350, 1_000)));
        assert_eq!(parse_ratio("16/10"), Some((16_000, 10_000)));
        assert_eq!(parse_ratio("4 x 3"), Some((4_000, 3_000)));
        assert_eq!(parse_ratio("2.39"), Some((2_390, 1_000)));
        for wrong in ["", "wide", "0:1", "1:0", "-4:3", "100:1"] {
            assert_eq!(parse_ratio(wrong), None, "{wrong}");
        }
        assert_eq!(AspectRatio::Custom.ratio("bad"), None, "unreadable keeps the media's own");
    }
}
