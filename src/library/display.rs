//! What the library views show for a media's tags (S4.3): the title, artist and album, with the
//! fallbacks for missing tags (the file name, an episode's name, "Unknown Artist", "Unknown
//! Album"), and a tag naming several artists split for the Artists view.

use std::path::Path;

use super::episode;
use crate::db::metadata::Metadata;

/// Shown for a song without an artist tag.
pub const UNKNOWN_ARTIST: &str = "Unknown Artist";
/// Shown for a song without an album tag.
pub const UNKNOWN_ALBUM: &str = "Unknown Album";
/// What separates artists in one tag (`A; B`, `A feat. B`), matched in any letter case. `&`
/// and `/` aren't here: they are part of too many names (`Simon & Garfunkel`, `AC/DC`).
pub const ARTIST_SEPARATORS: [&str; 5] = [";", " feat. ", " feat ", " ft. ", " featuring "];

/// The title to show for media at `path`: its title tag, else the episode its file name names,
/// else the file name.
pub fn title(metadata: &Metadata, path: &Path) -> String {
    if let Some(title) = &metadata.title {
        return title.clone();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    match episode::parse(&stem) {
        Some(episode) => episode.to_string(),
        None => path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
    }
}

/// The artist to show: the artist tag, else the album artist, else "Unknown Artist".
pub fn artist(metadata: &Metadata) -> &str {
    metadata.artist.as_deref().or(metadata.album_artist.as_deref()).unwrap_or(UNKNOWN_ARTIST)
}

/// The album to show: the album tag, else "Unknown Album".
pub fn album(metadata: &Metadata) -> &str {
    metadata.album.as_deref().unwrap_or(UNKNOWN_ALBUM)
}

/// The artists an artist tag names, split at `separators` (matched in any letter case), each
/// trimmed, empty ones left out.
pub fn split_artists(tag: &str, separators: &[&str]) -> Vec<String> {
    let lower = tag.to_ascii_lowercase();
    let mut cuts: Vec<(usize, usize)> = separators
        .iter()
        .flat_map(|separator| {
            let separator = separator.to_ascii_lowercase();
            let length = separator.len();
            lower
                .match_indices(&separator)
                .map(move |(at, _)| (at, at + length))
                .collect::<Vec<_>>()
        })
        .collect();
    cuts.sort_unstable();
    let mut artists = Vec::new();
    let mut from = 0;
    for (start, end) in cuts {
        if start >= from {
            artists.push(&tag[from..start]);
            from = end;
        }
    }
    artists.push(&tag[from..]);
    artists.into_iter().map(str::trim).filter(|artist| !artist.is_empty()).map(Into::into).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_fall_back_to_the_episode_then_the_file_name() {
        let tagged = Metadata { title: Some("Pilot".into()), ..Metadata::default() };
        assert_eq!(title(&tagged, Path::new("/tv/Show.S01E01.mkv")), "Pilot");
        let untagged = Metadata::default();
        assert_eq!(title(&untagged, Path::new("/tv/Show.S01E01.mkv")), "Show S01E01");
        assert_eq!(title(&untagged, Path::new("/m/holiday.mp4")), "holiday.mp4");
    }

    #[test]
    fn missing_artists_and_albums_are_unknown() {
        let untagged = Metadata::default();
        assert_eq!((artist(&untagged), album(&untagged)), (UNKNOWN_ARTIST, UNKNOWN_ALBUM));
        let compilation = Metadata { album_artist: Some("Various".into()), ..Metadata::default() };
        assert_eq!(artist(&compilation), "Various");
    }

    #[test]
    fn several_artists_in_one_tag_are_split() {
        assert_eq!(split_artists("A; B ;C", &ARTIST_SEPARATORS), ["A", "B", "C"]);
        assert_eq!(split_artists("A Feat. B", &ARTIST_SEPARATORS), ["A", "B"]);
        assert_eq!(split_artists("Simon & Garfunkel", &ARTIST_SEPARATORS), ["Simon & Garfunkel"]);
        assert_eq!(split_artists("AC/DC;", &ARTIST_SEPARATORS), ["AC/DC"]);
        assert_eq!(split_artists("A / B", &["/"]), ["A", "B"]);
    }
}
