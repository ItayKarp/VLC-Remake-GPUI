//! What the title bar and the OS window title say (S2.1.2, D8): the media's title while media
//! is loaded, otherwise the app's name.

use crate::app::media::Media;

/// The title when nothing is loaded, or when the file name is not to be shown.
pub const APP_TITLE: &str = "VLC Media Player";

/// Explicit bidi embeddings, overrides and isolates. Text shaping orders right-to-left text by
/// itself; a stray override from a bad tag would only reverse what follows it.
const BIDI_CONTROLS: [char; 9] = [
    '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}', '\u{2066}', '\u{2067}', '\u{2068}',
    '\u{2069}',
];

/// What is loaded, as far as the title cares.
pub struct Loaded<'a> {
    /// The media itself.
    pub media: &'a Media,
    /// Its title tag, if it has one.
    pub tag: Option<&'a str>,
}

/// The window title: the title tag of what's loaded, else its file name with the extension (or
/// its URL), else [`APP_TITLE`]. `show_file_name` off always gives [`APP_TITLE`].
pub fn window_title(loaded: Option<Loaded<'_>>, show_file_name: bool) -> String {
    let Some(loaded) = loaded.filter(|_| show_file_name) else {
        return APP_TITLE.to_owned();
    };
    let tagged = loaded.tag.map(clean).filter(|title| !title.is_empty());
    tagged
        .or_else(|| Some(clean(&loaded.media.display_name())).filter(|name| !name.is_empty()))
        .unwrap_or_else(|| APP_TITLE.to_owned())
}

/// `text` on one line: control characters (newlines and tabs too) become spaces, runs of
/// spaces become one, bidi formatting characters go, and the ends are trimmed.
fn clean(text: &str) -> String {
    let spaced = text
        .chars()
        .filter(|c| !BIDI_CONTROLS.contains(c))
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>();
    spaced.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn file(name: &str) -> Media {
        Media::File(PathBuf::from("videos").join(name))
    }

    fn title(media: &Media, tag: Option<&str>) -> String {
        window_title(Some(Loaded { media, tag }), true)
    }

    #[test]
    fn nothing_loaded_is_the_app() {
        assert_eq!(window_title(None, true), APP_TITLE);
    }

    #[test]
    fn the_tag_wins_over_the_file_name() {
        assert_eq!(title(&file("s01e04.mkv"), Some("The Wand S01E04")), "The Wand S01E04");
    }

    #[test]
    fn without_a_tag_the_file_name_keeps_its_extension() {
        assert_eq!(title(&file("The_Wand_S01E04.mp4"), None), "The_Wand_S01E04.mp4");
        assert_eq!(title(&file("clip.mp4"), Some("  \n ")), "clip.mp4");
    }

    #[test]
    fn urls_show_as_written() {
        let url = Media::Url("https://example.com/live.m3u8".into());
        assert_eq!(title(&url, None), "https://example.com/live.m3u8");
    }

    #[test]
    fn the_setting_off_hides_the_name() {
        let media = file("a.mkv");
        assert_eq!(window_title(Some(Loaded { media: &media, tag: Some("A") }), false), APP_TITLE);
    }

    #[test]
    fn bad_tags_come_out_on_one_clean_line() {
        let media = file("a.mkv");
        assert_eq!(title(&media, Some("Line one\nline\ttwo\u{7}")), "Line one line two");
        assert_eq!(title(&media, Some("\u{202E}abc\u{202C} 🎬")), "abc 🎬");
        assert_eq!(title(&media, Some("שלום עולם")), "שלום עולם");
    }
}
