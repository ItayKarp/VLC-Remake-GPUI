//! What bookmarks are called and how their places are written, kept apart from the window so it
//! can be tested.

use std::time::Duration;

use crate::app::media::Media;
use crate::db::bookmarks::Bookmark;

/// What a new bookmark is named after: the title tag, else the file's name without its
/// extension, else the stream's address. libVLC reports the file's name as the title of a file
/// without one, which counts as none.
pub fn media_title(tag: Option<&str>, media: &Media) -> String {
    let tag = tag.map(str::trim).filter(|tag| !tag.is_empty() && *tag != media.display_name());
    if let Some(tag) = tag {
        return tag.to_owned();
    }
    let stem = media.as_path().and_then(|path| path.file_stem());
    stem.map_or_else(|| media.display_name(), |stem| stem.to_string_lossy().into_owned())
}

/// A new bookmark's name: "<title> N", N one past the highest number already after the title,
/// so removing a bookmark never makes two share a name.
pub fn next_name(title: &str, existing: &[Bookmark]) -> String {
    let highest = existing
        .iter()
        .filter_map(|bookmark| bookmark.name.strip_prefix(title)?.strip_prefix(' '))
        .filter_map(|number| number.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{title} {}", highest.saturating_add(1))
}

/// A bookmark's place to the millisecond, `HH:MM:SS.mmm` (S8.3.4): the design's `00:00:42:023`
/// with the standard dot before the milliseconds, so it doesn't read as frames.
pub fn timestamp(position: Duration) -> String {
    let seconds = position.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    format!("{hours:02}:{minutes:02}:{seconds:02}.{:03}", position.subsec_millis())
}

/// `position` cut to whole milliseconds, as bookmarks are stored.
pub fn to_millisecond(position: Duration) -> Duration {
    Duration::from_millis(u64::try_from(position.as_millis()).unwrap_or(u64::MAX))
}

/// Where a bookmark at `position` goes in `existing`, sorted by position: `Ok` with the index
/// of the one already there, or `Err` with where to insert it.
pub fn slot(existing: &[Bookmark], position: Duration) -> Result<usize, usize> {
    existing.binary_search_by_key(&position, |bookmark| bookmark.position)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn named(name: &str) -> Bookmark {
        Bookmark { position: Duration::ZERO, name: name.into() }
    }

    #[test]
    fn the_title_is_the_tag_or_the_file_name_without_its_extension() {
        let file = Media::File(PathBuf::from("dir").join("Don't Blink.mkv"));
        assert_eq!(media_title(Some("DON'T BLINK"), &file), "DON'T BLINK");
        assert_eq!(media_title(Some("  "), &file), "Don't Blink");
        assert_eq!(media_title(None, &file), "Don't Blink");
        assert_eq!(media_title(Some("Don't Blink.mkv"), &file), "Don't Blink");
        let url = Media::Url("https://x/y".into());
        assert_eq!(media_title(None, &url), "https://x/y");
    }

    #[test]
    fn names_count_on_from_the_highest_number() {
        assert_eq!(next_name("Film", &[]), "Film 1");
        let existing = [named("Film 1"), named("Film 3"), named("Intro"), named("Film x")];
        assert_eq!(next_name("Film", &existing), "Film 4");
        assert_eq!(next_name("Fil", &[named("Film 7")]), "Fil 1");
    }

    #[test]
    fn timestamps_have_milliseconds_after_a_dot() {
        assert_eq!(timestamp(Duration::from_millis(42_023)), "00:00:42.023");
        assert_eq!(timestamp(Duration::from_millis(3_723_004)), "01:02:03.004");
        assert_eq!(timestamp(Duration::from_hours(100)), "100:00:00.000");
    }

    #[test]
    fn positions_are_cut_to_the_millisecond() {
        assert_eq!(
            to_millisecond(Duration::from_micros(42_023_999)),
            Duration::from_millis(42_023)
        );
    }

    #[test]
    fn a_place_already_bookmarked_is_found() {
        let at = |millis| Bookmark { position: Duration::from_millis(millis), name: String::new() };
        let existing = [at(10), at(30)];
        assert_eq!(slot(&existing, Duration::from_millis(30)), Ok(1));
        assert_eq!(slot(&existing, Duration::from_millis(20)), Err(1));
    }
}
