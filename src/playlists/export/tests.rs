use std::fs;
use std::time::Duration;

use super::*;
use crate::opening::Cancel;
use crate::opening::kind::PlaylistFormat;
use crate::opening::playlist;
use crate::opening::scratch::scratch_dir;

fn items(dir: &Path) -> Vec<Item> {
    vec![
        Item {
            media: Media::File(dir.join("music").join("Ünïcode & <song>.mp3")),
            duration: Some(Duration::from_millis(123_456)),
        },
        Item { media: Media::File(dir.join("#hash.mp3")), duration: None },
        Item { media: Media::Url("http://radio.example/live?a=1&b=2".into()), duration: None },
    ]
}

/// What the app reads back from `destination` after exporting to it.
fn round_trip(format: Format, paths: PathStyle) -> (String, Vec<Media>, Vec<Item>) {
    let dir = scratch_dir();
    let items = items(&dir);
    let destination = dir.join(format!("Mix.{}", format.extension()));
    let export = Export { title: "Mix", items: &items, paths };
    write(&destination, &render(&export, format, &destination)).unwrap();
    let text = fs::read_to_string(&destination).unwrap();
    let parsed_format = match format {
        Format::M3u8 => PlaylistFormat::M3u,
        Format::Xspf => PlaylistFormat::Xspf,
    };
    let read = playlist::expand(&destination, parsed_format, &Cancel::default()).unwrap();
    (text, read, items)
}

fn medias(items: &[Item]) -> Vec<Media> {
    items.iter().map(|item| item.media.clone()).collect()
}

#[test]
fn m3u8_round_trips_with_relative_and_absolute_paths() {
    for paths in [PathStyle::Relative, PathStyle::Absolute] {
        let (text, read, items) = round_trip(Format::M3u8, paths);
        assert_eq!(read, medias(&items), "{paths:?}");
        assert!(text.starts_with("#EXTM3U\n#PLAYLIST:Mix\n#EXTINF:123,Ünïcode & <song>\n"));
        assert!(text.contains("#EXTINF:-1,#hash\n"));
        assert_eq!(text.contains("music/Ünïcode"), paths == PathStyle::Relative || cfg!(unix));
    }
    let (text, ..) = round_trip(Format::M3u8, PathStyle::Relative);
    assert!(text.contains("\n./#hash.mp3\n"), "{text}");
    assert!(text.contains(&format!("\nmusic{}Ünïcode", std::path::MAIN_SEPARATOR)), "{text}");
}

#[test]
fn xspf_round_trips_with_relative_and_absolute_paths() {
    for paths in [PathStyle::Relative, PathStyle::Absolute] {
        let (text, read, items) = round_trip(Format::Xspf, paths);
        assert_eq!(read, medias(&items), "{paths:?}");
        assert!(text.contains("<duration>123456</duration>"));
        assert!(text.contains("<title>Ünïcode &amp; &lt;song&gt;</title>"));
        assert!(text.contains("?a=1&amp;b=2"));
        let relative = text.contains("<location>music/%C3%9C");
        assert_eq!(relative, paths == PathStyle::Relative, "{text}");
    }
}

#[test]
fn relative_paths_climb_out_of_the_folder() {
    let root = std::env::temp_dir();
    let path = root.join("a").join("b.mp3");
    let folder = root.join("lists").join("old");
    let expected: PathBuf = ["..", "..", "a", "b.mp3"].iter().collect();
    assert_eq!(relative_path(&path, &folder), Some(expected));
    assert_eq!(relative_path(Path::new("b.mp3"), &folder), None);
}

#[test]
fn the_typed_extension_picks_the_format() {
    assert_eq!(Format::for_path(Path::new("x.XSPF"), Format::M3u8), Format::Xspf);
    assert_eq!(Format::for_path(Path::new("x.m3u"), Format::Xspf), Format::M3u8);
    assert_eq!(Format::for_path(Path::new("x"), Format::Xspf), Format::Xspf);
}

#[test]
fn a_failed_write_leaves_nothing_behind() {
    let dir = scratch_dir();
    let destination = dir.join("missing").join("Mix.m3u8");
    assert!(write(&destination, "#EXTM3U\n").is_err());
    assert!(!destination.exists());
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn a_read_only_folder_keeps_the_old_file() {
    use std::os::unix::fs::PermissionsExt as _;

    let dir = scratch_dir();
    let destination = dir.join("Mix.m3u8");
    fs::write(&destination, "old").unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
    let written = write(&destination, "new");
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(written.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(fs::read_to_string(&destination).unwrap(), "old");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn writing_replaces_an_existing_file() {
    let dir = scratch_dir();
    let destination = dir.join("Mix.m3u8");
    fs::write(&destination, "old").unwrap();
    write(&destination, "new").unwrap();
    assert_eq!(fs::read_to_string(&destination).unwrap(), "new");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
}
