//! The subtitle settings as the engine takes them (S5.8.3): the style the player's libVLC starts
//! with, and how each new media finds and reads subtitle files.

use crate::engine::subtitle_style::{BLACK, SubtitleLoading, SubtitleStyle, TextSize, WHITE};
use crate::settings::{self, registry};
use crate::settings::{SizeValues, SubtitleSettings};

/// The cache folder subtitle files converted to UTF-8 go in.
const CONVERTED_DIR_NAME: &str = "subtitles";
/// The highest opacity percentage, fully opaque.
const FULL_PERCENT: u32 = 100;

/// The style plain-text subtitles are drawn in.
pub(crate) fn style(subtitles: &SubtitleSettings) -> SubtitleStyle {
    let size = match subtitles.size_values {
        SizeValues::Relative => TextSize::Relative(subtitles.relative_size),
        SizeValues::Pixels => TextSize::Pixels(subtitles.pixel_size),
    };
    SubtitleStyle {
        font: subtitles.font.trim().to_owned(),
        size,
        bold: subtitles.bold,
        color: registry::hex_color(&subtitles.text_color).unwrap_or(WHITE),
        opacity: opacity(subtitles.text_opacity_percent),
        outline: subtitles.outline,
        outline_color: registry::hex_color(&subtitles.outline_color).unwrap_or(BLACK),
        shadow: subtitles.shadow,
    }
}

/// Which subtitle files load with new media, their encoding, and where copies converted to
/// UTF-8 go (S6.1.6).
pub(super) fn loading(subtitles: &SubtitleSettings) -> SubtitleLoading {
    SubtitleLoading {
        files: subtitles.load_files.then_some(subtitles.file_matching),
        encoding: subtitles.text_encoding.trim().to_owned(),
        converted: settings::cache_dir().map(|dir| dir.join(CONVERTED_DIR_NAME)),
    }
}

/// A 0–100% opacity as libVLC's 0–255.
fn opacity(percent: u32) -> u8 {
    let percent = percent.min(FULL_PERCENT);
    u8::try_from(percent * u32::from(u8::MAX) / FULL_PERCENT).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::subtitle_style::{FileMatching, RelativeSize};

    #[test]
    fn the_default_settings_are_the_default_style() {
        let subtitles = SubtitleSettings::default();
        assert_eq!(style(&subtitles), SubtitleStyle::default());
        assert_eq!(loading(&subtitles).files, Some(FileMatching::TitleWithExtra));
    }

    #[test]
    fn sizes_colors_and_opacity_convert() {
        let subtitles = SubtitleSettings {
            size_values: SizeValues::Pixels,
            pixel_size: 40,
            relative_size: RelativeSize::Large,
            text_color: "#FF0".into(),
            text_opacity_percent: 50,
            load_files: false,
            ..SubtitleSettings::default()
        };
        let styled = style(&subtitles);
        assert_eq!(styled.size, TextSize::Pixels(40));
        assert_eq!(styled.color, 0xFF_FF00);
        assert_eq!(styled.opacity, 127);
        assert_eq!(loading(&subtitles).files, None);
    }
}
