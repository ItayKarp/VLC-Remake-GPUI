//! Deciding and converting subtitle files' encodings.

use std::fmt::Write as _;
use std::sync::atomic::{AtomicU32, Ordering};

use encoding_rs::{UTF_16LE, WINDOWS_1251, WINDOWS_1255};

use super::*;

/// A few cues of `line`, as an SRT file holds them.
fn srt(line: &str) -> String {
    let mut text = String::new();
    for n in 1..=12 {
        writeln!(text, "{n}\n00:00:{n:02},000 --> 00:00:{n:02},900\n{line}\n").unwrap();
    }
    text
}

const HEBREW: &str = "שלום, מה שלומך היום? אני בסדר גמור, תודה רבה לך על השאלה.";
const RUSSIAN: &str = "Привет, как дела сегодня? Всё хорошо, спасибо большое за вопрос.";

/// An empty folder of its own for one test.
fn scratch() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "rvlc-subtitle-text-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Writes `text` in `encoding` as `name` in `dir`.
fn write(dir: &Path, name: &str, text: &str, encoding: &'static Encoding) -> PathBuf {
    let path = dir.join(name);
    let (bytes, _, _) = encoding.encode(text);
    fs::write(&path, bytes).unwrap();
    path
}

#[test]
fn utf8_and_marked_files_load_as_they_are() {
    let dir = scratch();
    let utf8 = write(&dir, "Movie.he.srt", &srt(HEBREW), UTF_8);
    let prepared = prepare(&utf8, EncodingChoice::Guess, Some(&dir.join("cache")));
    assert_eq!(prepared, Prepared { path: utf8, encoding: Some(UTF_8) });

    let marked = dir.join("Movie.ru.srt");
    let units = srt(RUSSIAN).encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>();
    fs::write(&marked, [&[0xFF, 0xFE][..], &units].concat()).unwrap();
    let prepared = prepare(&marked, EncodingChoice::Guess, Some(&dir.join("cache")));
    assert_eq!(prepared, Prepared { path: marked, encoding: Some(UTF_16LE) });
}

#[test]
fn a_code_page_is_guessed_and_converted_to_marked_utf8_under_the_same_name() {
    let dir = scratch();
    for (name, text, encoding) in
        [("Movie.he.srt", HEBREW, WINDOWS_1255), ("Movie.ru.srt", RUSSIAN, WINDOWS_1251)]
    {
        let original = write(&dir, name, &srt(text), encoding);
        let prepared = prepare(&original, EncodingChoice::Guess, Some(&dir.join("cache")));
        assert_eq!(prepared.encoding, Some(encoding), "{name}");
        assert_eq!(prepared.path.file_name(), original.file_name());
        assert!(prepared.path.starts_with(dir.join("cache")));
        let copy = fs::read(&prepared.path).unwrap();
        assert!(copy.starts_with(UTF8_BOM));
        assert_eq!(std::str::from_utf8(&copy[UTF8_BOM.len()..]).unwrap(), srt(text));
    }
}

#[test]
fn a_chosen_encoding_wins_over_the_guess() {
    let dir = scratch();
    let original = write(&dir, "Movie.srt", &srt(RUSSIAN), WINDOWS_1251);
    let chosen = EncodingChoice::Use(WINDOWS_1255);
    let prepared = prepare(&original, chosen, Some(&dir.join("cache")));
    assert_eq!(prepared.encoding, Some(WINDOWS_1255));
    assert_ne!(prepared.path, original);
}

#[test]
fn the_setting_pictures_and_unreadable_files_load_as_they_are() {
    let dir = scratch();
    let cache = dir.join("cache");
    let hebrew = write(&dir, "Movie.srt", &srt(HEBREW), WINDOWS_1255);
    assert_eq!(prepare(&hebrew, EncodingChoice::Setting, Some(&cache)).path, hebrew);
    let pictures = dir.join("Movie.sup");
    fs::write(&pictures, b"PG\x00\x01").unwrap();
    assert_eq!(prepare(&pictures, EncodingChoice::Guess, Some(&cache)).path, pictures);
    let binary = dir.join("Movie.sub");
    fs::write(&binary, b"\x00\x00\x01\xBA").unwrap();
    assert_eq!(prepare(&binary, EncodingChoice::Guess, Some(&cache)).path, binary);
    let missing = dir.join("gone.srt");
    assert_eq!(prepare(&missing, EncodingChoice::Guess, Some(&cache)).path, missing);
    let no_cache = prepare(&hebrew, EncodingChoice::Guess, None);
    assert_eq!(no_cache, Prepared { path: hebrew, encoding: Some(WINDOWS_1255) });
}

#[test]
fn decoding_reads_converted_copies_and_the_setting_s_encoding() {
    let dir = scratch();
    let hebrew = write(&dir, "Movie.srt", &srt(HEBREW), WINDOWS_1255);
    let converted = prepare(&hebrew, EncodingChoice::Guess, Some(&dir.join("cache")));
    assert!(decode(&converted, "").unwrap().contains(HEBREW));
    let as_is = prepare(&hebrew, EncodingChoice::Setting, None);
    assert!(decode(&as_is, "Windows-1255").unwrap().contains(HEBREW));
    let picture = Prepared { path: dir.join("Movie.sup"), encoding: None };
    assert_eq!(decode(&picture, ""), None);
}
