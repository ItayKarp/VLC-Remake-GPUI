use std::fs;

use super::*;
use crate::opening::scratch::scratch_dir;

fn files(items: &[Media]) -> Vec<String> {
    items.iter().map(Media::display_name).collect()
}

#[test]
fn nested_playlists_expand_in_place() {
    let dir = scratch_dir();
    fs::write(dir.join("outer.m3u"), "a.mp3\ninner.PLS\nhttp://x/remote.m3u\nd.mp3\n").unwrap();
    fs::write(dir.join("inner.PLS"), "[playlist]\nFile1=b.mp3\nFile2=c.mp3\n").unwrap();
    let items = expand(&dir.join("outer.m3u"), PlaylistFormat::M3u, &Cancel::default()).unwrap();
    assert_eq!(files(&items), ["a.mp3", "b.mp3", "c.mp3", "http://x/remote.m3u", "d.mp3"]);
}

#[test]
fn cycles_are_expanded_once() {
    let dir = scratch_dir();
    fs::write(dir.join("a.m3u"), "1.mp3\nb.m3u\n").unwrap();
    fs::write(dir.join("b.m3u"), "2.mp3\na.m3u\nb.m3u\n").unwrap();
    let items = expand(&dir.join("a.m3u"), PlaylistFormat::M3u, &Cancel::default()).unwrap();
    assert_eq!(files(&items), ["1.mp3", "2.mp3"]);
}

#[test]
fn nesting_stops_at_the_depth_limit() {
    let dir = scratch_dir();
    for level in 0..MAX_DEPTH + 2 {
        fs::write(dir.join(format!("{level}.m3u")), format!("{level}.mp3\n{}.m3u\n", level + 1))
            .unwrap();
    }
    let items = expand(&dir.join("0.m3u"), PlaylistFormat::M3u, &Cancel::default()).unwrap();
    assert_eq!(items.len(), MAX_DEPTH);
}

#[test]
fn a_local_hls_playlist_is_a_stream() {
    let dir = scratch_dir();
    let path = dir.join("live.m3u8");
    fs::write(&path, "#EXTM3U\n#EXT-X-TARGETDURATION:6\nseg.ts\n").unwrap();
    let items = expand(&path, PlaylistFormat::M3u, &Cancel::default()).unwrap();
    assert_eq!(items, [Media::File(path)]);
}

#[test]
fn empty_and_missing_playlists_fail() {
    let dir = scratch_dir();
    fs::write(dir.join("empty.m3u"), "#EXTM3U\n# nothing\n").unwrap();
    let cancel = Cancel::default();
    assert_eq!(
        expand(&dir.join("empty.m3u"), PlaylistFormat::M3u, &cancel),
        Err(PlaylistError::Empty)
    );
    assert_eq!(
        expand(&dir.join("missing.m3u"), PlaylistFormat::M3u, &cancel),
        Err(PlaylistError::Unreadable(io::ErrorKind::NotFound))
    );
}
