//! [`MediaOptions`] as libVLC's per-media input options: `preferred-resolution` for adaptive
//! streams, `menu-language` for DVD and Blu-ray menus, and `input-fast-seek` for keyframe seeks.
//! Options left at libVLC's own default (best quality, the disc's language, precise seeks) are
//! not added.

use super::api::Media;
use super::worker::Worker;
use crate::engine::media_options::MediaOptions;

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
    added
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::language::Language;

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
        };
        assert_eq!(
            libvlc_options(&options),
            [":preferred-resolution=720", ":menu-language=fr", ":input-fast-seek"]
        );
    }
}
