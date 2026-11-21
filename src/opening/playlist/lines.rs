//! The line-based playlist formats: M3U (and HLS, which shares its syntax), PLS and CUE.

use super::Parsed;
use super::text::Entry;

/// Tags only HLS playlists carry. A local `.m3u8` with them is a stream to play, not a list.
const HLS_TAGS: &[&str] = &[
    "#EXT-X-TARGETDURATION",
    "#EXT-X-STREAM-INF",
    "#EXT-X-MEDIA-SEQUENCE",
    "#EXT-X-PLAYLIST-TYPE",
    "#EXT-X-KEY",
    "#EXT-X-MAP",
];

/// M3U and M3U8: every line that isn't blank or a `#` comment is an entry.
pub fn m3u(text: &str) -> Parsed {
    let lines = || text.lines().map(str::trim);
    if lines().any(|line| HLS_TAGS.iter().any(|tag| starts_with_ignore_case(line, tag))) {
        return Parsed::Stream;
    }
    let entries = lines().filter(|line| !line.is_empty() && !line.starts_with('#'));
    Parsed::Entries(entries.map(|line| Entry::Location(line.to_owned())).collect())
}

/// PLS: `FileN=…` keys, in the order of N, whatever order the lines are in.
pub fn pls(text: &str) -> Parsed {
    let mut numbered: Vec<(u32, &str)> = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.trim().split_once('=')?;
            let key = key.trim();
            if !starts_with_ignore_case(key, "File") {
                return None;
            }
            Some((key["File".len()..].parse().ok()?, value.trim()))
        })
        .collect();
    numbered.sort_by_key(|(number, _)| *number);
    Parsed::Entries(numbered.into_iter().map(|(_, value)| Entry::Location(value.into())).collect())
}

/// CUE: the audio files its `FILE "name" TYPE` lines name, each once. The tracks inside one
/// file aren't split out: an item has no start and end time yet (Stage 3).
pub fn cue(text: &str) -> Parsed {
    let mut entries: Vec<Entry> = Vec::new();
    for line in text.lines().map(str::trim) {
        if !starts_with_ignore_case(line, "FILE ") {
            continue;
        }
        let Some(name) = cue_file_name(line["FILE ".len()..].trim()) else {
            continue;
        };
        let entry = Entry::Location(name.to_owned());
        if !entries.contains(&entry) {
            entries.push(entry);
        }
    }
    Parsed::Entries(entries)
}

/// The file name in the rest of a `FILE` line: quoted, or everything before the type word.
fn cue_file_name(rest: &str) -> Option<&str> {
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted.split_once('"').map(|(name, _)| name);
    }
    match rest.rsplit_once(char::is_whitespace) {
        Some((name, _type)) => Some(name.trim_end()),
        None => Some(rest).filter(|name| !name.is_empty()),
    }
}

fn starts_with_ignore_case(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len()).is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(parsed: Parsed) -> Vec<String> {
        let Parsed::Entries(entries) = parsed else { panic!("expected entries") };
        entries
            .into_iter()
            .map(|entry| match entry {
                Entry::Location(text) | Entry::Uri(text) => text,
            })
            .collect()
    }

    #[test]
    fn m3u_skips_comments_and_blank_lines() {
        let text = "#EXTM3U\r\n#EXTINF:123,Artist - Title\r\nmusic/a.mp3\r\n\r\nhttp://x/y.ogg\r\n";
        assert_eq!(entries(m3u(text)), ["music/a.mp3", "http://x/y.ogg"]);
    }

    #[test]
    fn hls_is_a_stream() {
        let text = "#EXTM3U\n#EXT-X-TARGETDURATION:10\n#EXTINF:10,\nseg0.ts\n";
        assert_eq!(m3u(text), Parsed::Stream);
    }

    #[test]
    fn pls_orders_by_number() {
        let text =
            "[playlist]\nFile2=b.mp3\nTitle1=A\nfile1=a.mp3\nNumberOfEntries=2\nFile10=c.mp3";
        assert_eq!(entries(pls(text)), ["a.mp3", "b.mp3", "c.mp3"]);
    }

    #[test]
    fn cue_lists_each_file_once() {
        let text = "REM DATE 1999\nFILE \"Disc 1.flac\" WAVE\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n\
                    FILE Disc 2.wav WAVE\nFILE \"Disc 1.flac\" WAVE\nFILE \"unterminated";
        assert_eq!(entries(cue(text)), ["Disc 1.flac", "Disc 2.wav"]);
    }
}
