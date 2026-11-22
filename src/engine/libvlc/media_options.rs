//! [`MediaOptions`] as libVLC's per-media input options: `preferred-resolution` for adaptive
//! streams, `menu-language` for DVD and Blu-ray menus, `input-fast-seek` for keyframe seeks,
//! `no-video` for sound only, `no-audio` for a silent picture, the `adjust` filter for the picture, and `deinterlace` with
//! `deinterlace-mode`, and the subtitle files and encoding (`subtitle_style.rs`). Options left at libVLC's own default (best quality, the disc's language,
//! precise seeks, video on, an unchanged picture, automatic deinterlacing) are not added.

use super::api::Media;
use super::worker::Worker;
use crate::engine::media_options::{Deinterlace, DeinterlaceMethod, MediaOptions, Picture};

impl Worker {
    /// Adds the media options the UI set to `media`, which is live and not yet playing.
    pub(super) fn add_media_options(&self, media: *mut Media) {
        let options = libvlc_options(&self.media_options);
        let options: Vec<&str> = options.iter().map(String::as_str).collect();
        self.add_option(media, &options);
    }
}

/// The per-media options `options` stand for.
fn libvlc_options(options: &MediaOptions) -> Vec<String> {
    let mut added = Vec::new();
    if let Some(lines) = options.max_lines {
        added.push(format!(":preferred-resolution={lines}"));
    }
    if let Some(language) = &options.menu_language {
        added.push(format!(":menu-language={}", language.code()));
    }
    if !options.precise_seeks {
        added.push(":input-fast-seek".to_owned());
    }
    if !options.video {
        added.push(":no-video".to_owned());
    }
    if !options.audio {
        added.push(":no-audio".to_owned());
    }
    added.extend(picture_options(&options.picture));
    added.extend(deinterlace_options(options.deinterlace.mode, options.deinterlace.method));
    added.extend(super::subtitle_style::media_options(&options.subtitles));
    added
}

/// The `adjust` filter set to `picture`, unless it changes nothing.
fn picture_options(picture: &Picture) -> Vec<String> {
    if picture.is_neutral() {
        return Vec::new();
    }
    vec![
        ":video-filter=adjust".to_owned(),
        format!(":hue={}", picture.hue),
        format!(":brightness={}", picture.brightness),
        format!(":contrast={}", picture.contrast),
        format!(":saturation={}", picture.saturation),
        format!(":gamma={}", picture.gamma),
    ]
}

/// `deinterlace` (libVLC: -1 automatic, 0 off, 1 on) and its method, where not the default.
fn deinterlace_options(mode: Deinterlace, method: DeinterlaceMethod) -> Vec<String> {
    let mut added = Vec::new();
    match mode {
        Deinterlace::Automatic => {}
        Deinterlace::On => added.push(":deinterlace=1".to_owned()),
        Deinterlace::Off => return vec![":deinterlace=0".to_owned()],
    }
    if method != DeinterlaceMethod::Automatic {
        added.push(format!(":deinterlace-mode={}", method.libvlc_name()));
    }
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::language::Language;
    use crate::engine::media_options::Deinterlacing;

    #[test]
    fn the_defaults_add_nothing() {
        assert_eq!(libvlc_options(&MediaOptions::default()), Vec::<String>::new());
    }

    #[test]
    fn each_choice_becomes_its_option() {
        let options = MediaOptions {
            max_lines: Some(720),
            menu_language: Language::parse("fre"),
            precise_seeks: false,
            ..MediaOptions::default()
        };
        assert_eq!(
            libvlc_options(&options),
            [":preferred-resolution=720", ":menu-language=fr", ":input-fast-seek"]
        );
    }

    #[test]
    fn video_off_decodes_none() {
        let options = MediaOptions { video: false, ..MediaOptions::default() };
        assert_eq!(libvlc_options(&options), [":no-video"]);
    }

    #[test]
    fn a_changed_picture_adds_the_adjust_filter_with_every_value() {
        let picture = Picture { hue: -30., saturation: 0., ..Picture::NEUTRAL };
        let options = MediaOptions { picture, ..MediaOptions::default() };
        assert_eq!(
            libvlc_options(&options),
            [
                ":video-filter=adjust",
                ":hue=-30",
                ":brightness=1",
                ":contrast=1",
                ":saturation=0",
                ":gamma=1",
            ]
        );
    }

    #[test]
    fn deinterlacing_names_its_mode_and_method() {
        let on = Deinterlacing { mode: Deinterlace::On, method: DeinterlaceMethod::Yadif2x };
        let options = MediaOptions { deinterlace: on, ..MediaOptions::default() };
        assert_eq!(libvlc_options(&options), [":deinterlace=1", ":deinterlace-mode=yadif2x"]);
        let off = Deinterlacing { mode: Deinterlace::Off, method: DeinterlaceMethod::Blend };
        let options = MediaOptions { deinterlace: off, ..MediaOptions::default() };
        assert_eq!(libvlc_options(&options), [":deinterlace=0"]);
    }
}
