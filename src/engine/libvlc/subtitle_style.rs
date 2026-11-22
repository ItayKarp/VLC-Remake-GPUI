//! [`SubtitleStyle`] as libVLC instance arguments for the `freetype` text renderer, and
//! [`SubtitleLoading`] as per-media options. Values left at libVLC's defaults add nothing.

use crate::engine::subtitle_style::{
    BLACK, OPAQUE, Outline, SubtitleLoading, SubtitleStyle, TextSize, WHITE,
};

/// The renderer's arguments for `style`.
pub fn arguments(style: &SubtitleStyle) -> Vec<String> {
    let mut arguments = Vec::new();
    if !style.font.trim().is_empty() {
        arguments.push(format!("--freetype-font={}", style.font.trim()));
    }
    match style.size {
        TextSize::Relative(size) if size.libvlc_value() != 0 => {
            arguments.push(format!("--freetype-rel-fontsize={}", size.libvlc_value()));
        }
        TextSize::Pixels(pixels) if pixels > 0 => {
            arguments.push(format!("--freetype-fontsize={pixels}"));
        }
        _ => {}
    }
    if style.bold {
        arguments.push("--freetype-bold".to_owned());
    }
    let numbers = [
        ("color", style.color, WHITE),
        ("opacity", u32::from(style.opacity), u32::from(OPAQUE)),
        ("outline-thickness", style.outline.libvlc_value(), Outline::Normal.libvlc_value()),
        ("outline-color", style.outline_color, BLACK),
    ];
    for (name, value, default) in numbers {
        if value != default {
            arguments.push(format!("--freetype-{name}={value}"));
        }
    }
    if !style.shadow {
        arguments.push("--freetype-shadow-opacity=0".to_owned());
    }
    arguments
}

/// The per-media options for `loading`: the encoding. Which files load is the player's
/// ([`crate::engine::sidecar`]); libVLC's own detection is off.
pub fn media_options(loading: &SubtitleLoading) -> Vec<String> {
    let encoding =
        (!loading.encoding.is_empty()).then(|| format!(":subsdec-encoding={}", loading.encoding));
    encoding.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::subtitle_style::{FileMatching, RelativeSize};

    #[test]
    fn the_default_style_and_loading_add_nothing() {
        assert_eq!(arguments(&SubtitleStyle::default()), Vec::<String>::new());
        let loading = SubtitleLoading {
            files: Some(FileMatching::TitleWithExtra),
            ..SubtitleLoading::default()
        };
        assert_eq!(media_options(&loading), Vec::<String>::new());
    }

    #[test]
    fn each_change_is_its_option() {
        let style = SubtitleStyle {
            font: "Avenir Next".into(),
            size: TextSize::Relative(RelativeSize::Large),
            bold: true,
            color: 0x00FF_FF00,
            opacity: 128,
            outline: Outline::None,
            outline_color: 0x0000_00FF,
            shadow: false,
        };
        assert_eq!(
            arguments(&style),
            [
                "--freetype-font=Avenir Next",
                "--freetype-rel-fontsize=12",
                "--freetype-bold",
                "--freetype-color=16776960",
                "--freetype-opacity=128",
                "--freetype-outline-thickness=0",
                "--freetype-outline-color=255",
                "--freetype-shadow-opacity=0",
            ]
        );
        let pixels = SubtitleStyle { size: TextSize::Pixels(32), ..SubtitleStyle::default() };
        assert_eq!(arguments(&pixels), ["--freetype-fontsize=32"]);
    }

    #[test]
    fn an_encoding_is_its_option() {
        let encoding =
            SubtitleLoading { encoding: "Windows-1255".into(), ..SubtitleLoading::default() };
        assert_eq!(media_options(&encoding), [":subsdec-encoding=Windows-1255"]);
    }
}

#[cfg(test)]
mod libvlc_tests {
    use super::*;
    use crate::engine::libvlc::{ARGUMENTS, library, new_instance, runtime};
    use crate::engine::subtitle_style::RelativeSize;

    #[test]
    #[ignore = "needs libVLC"]
    fn libvlc_takes_every_style_option() {
        // SAFETY: run on its own, as the other libVLC smoke tests.
        unsafe { runtime::prepare_environment() };
        let vlc = library().expect("libVLC loads");
        let style = SubtitleStyle {
            font: "Helvetica".into(),
            size: TextSize::Relative(RelativeSize::Small),
            bold: true,
            color: 0x00FF_0000,
            opacity: 200,
            outline: Outline::Thick,
            outline_color: 0x0000_FF00,
            shadow: false,
        };
        let mut options = arguments(&style);
        options.push("--freetype-fontsize=30".to_owned());
        let all: Vec<&str> =
            ARGUMENTS.iter().copied().chain(options.iter().map(String::as_str)).collect();
        let instance = new_instance(&vlc, &all).expect("libVLC takes the options");
        // SAFETY: the instance is live, unused and released once.
        unsafe { (vlc.api.libvlc_release)(instance) };
    }
}
