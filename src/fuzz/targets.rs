//! The parsers [`super::run`] fuzzes, each from a few real inputs. The web remote's request
//! reader is fuzzed beside it (`remote/http.rs`), since its module is private. VLC reads no
//! `.nfo` file itself: the library only recognizes the extension to leave the file out.

use std::path::Path;
use std::time::Duration;

use super::run;
use crate::engine::cues;
use crate::opening::kind::PlaylistFormat;
use crate::opening::playlist;
use crate::podcasts::feed;
use crate::settings::{LocalState, Preferences};
use crate::updates::manifest::Manifest;

const SUBRIP: &[u8] = concat!(
    "1\n00:00:01,000 --> 00:00:02,500\n<i>Hello</i> <font color=\"#ff0000\">world</font>\n",
    "\n2\n00:00:03,000 --> 00:00:04,000 X1:10 X2:20\n",
    "Line one\nLine two\n",
)
.as_bytes();
const WEBVTT: &[u8] = concat!(
    "WEBVTT\n\nNOTE x\n\n00:01.000 --> 00:02.000 align:start line:10%\n",
    "<v Bob><b>Hi</b> <c.red>there</c>\n\nid\n01:00:00.000 --> 01:00:01.000\n",
    "&amp; &lt;\n",
)
.as_bytes();
const ASS: &[u8] = concat!(
    "[Script Info]\nPlayResX: 1920\nPlayResY: 1080\n",
    "\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, Bold, Italic, Alignment\n",
    "Style: Default,Arial,40,&H00FFFFFF,0,0,2\n\n[Events]\n",
    "Format: Layer, Start, End, Style, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,{\\an8\\i1",
    "\\c&H0000FF&}Top\\Nline{\\i0}\n",
)
.as_bytes();
const MICRODVD: &[u8] = b"{1}{1}23.976\n{24}{48}Hello|{y:i}world\n{100}{}open\n";

const M3U: &[u8] = concat!(
    "#EXTM3U\n#EXTINF:123,Artist - Title\nmusic/a b.mp3\n",
    "https://example.com/live.m3u8\n#EXT-X-TARGETDURATION:6\n",
)
.as_bytes();
const PLS: &[u8] = concat!(
    "[playlist]\nNumberOfEntries=2\nFile1=a.mp3\nTitle1=A\n",
    "Length1=-1\nFile2=http://x/y\nVersion=2\n",
)
.as_bytes();
const CUE: &[u8] = concat!(
    "PERFORMER \"Band\"\nTITLE \"Album\"\nFILE \"album.flac\" WAVE\n",
    "  TRACK 01 AUDIO\n    TITLE \"One\"\n    INDEX 01 00:00:00\n",
    "  TRACK 02 AUDIO\n    INDEX 01 03:15:42\n",
)
.as_bytes();
const XSPF: &[u8] = concat!(
    "<?xml version=\"1.0\"?><playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\"><trackList><tr",
    "ack><location>music/a%20b.mp3</location><title>A &amp; B</title></track><track><location>file",
    ":///tmp/x.mkv</location></track></trackList></playlist>",
)
.as_bytes();
const ASX: &[u8] = concat!(
    "<ASX version=\"3.0\"><ENTRY><REF HREF=\"mms://x/y\"/><TITLE>T</TITLE></ENTRY><Entry><ref href",
    "='a.wma'/></Entry></ASX>",
)
.as_bytes();

const RSS: &[u8] = concat!(
    "<?xml version=\"1.0\"?><rss version=\"2.0\" xmlns:itunes=\"http://www.itunes.com/dtds/podcast",
    "-1.0.dtd\"><channel><title>Show</title><itunes:image href=\"https://x/c.jpg\"/><item><title>E",
    "p 1</title><guid>1</guid><pubDate>Mon, 02 Jan 2023 10:00:00 +0000</pubDate><enclosure url=\"h",
    "ttps://x/1.mp3\" length=\"10\" type=\"audio/mpeg\"/><itunes:duration>01:02:03</itunes:duratio",
    "n></item></channel></rss>",
)
.as_bytes();
const ATOM: &[u8] = concat!(
    "<feed xmlns=\"http://www.w3.org/2005/Atom\"><title>T</title><entry><id>e</id><title>E</title>",
    "<updated>2023-01-02T10:00:00Z</updated><link rel=\"enclosure\" href=\"https://x/e.mp3\" type=",
    "\"audio/mpeg\"/></entry></feed>",
)
.as_bytes();

const SETTINGS: &[u8] = concat!(
    "schema_version = 2\n\n[player]\nvolume = 80\n\n",
    "[privacy]\nsave_recently_played = true\nnever_remember = \"*.secret\"\n",
    "\n[network]\nhttp_proxy = \"http://proxy:3128\"\n",
)
.as_bytes();
const MANIFEST: &[u8] = concat!(
    "{\"version\":\"1.2.3\",\"release_page\":\"https://github.com/o/r/releases/tag/v1.2.3\",\"pack",
    "ages\":[{\"format\":\"macos-arm64\",\"url\":\"https://x/a.zip\",\"size\":10,\"sha256\":\"00\"",
    "}]}",
)
.as_bytes();

#[test]
fn subtitles() {
    let seeds = [SUBRIP, WEBVTT, ASS, MICRODVD];
    run("subtitles", &seeds, |input| {
        let text = String::from_utf8_lossy(input);
        for extension in ["srt", "vtt", "ass", "ssa", "sub", "txt"] {
            let Some(track) = cues::parse(Path::new(&format!("a.{extension}")), &text) else {
                continue;
            };
            for cue in track.at(Duration::from_secs(1)) {
                cue.text();
            }
        }
    });
}

#[test]
fn playlists() {
    let formats = [
        PlaylistFormat::M3u,
        PlaylistFormat::Pls,
        PlaylistFormat::Cue,
        PlaylistFormat::Xspf,
        PlaylistFormat::Asx,
    ];
    run("playlists", &[M3U, PLS, CUE, XSPF, ASX], |input| {
        let text = String::from_utf8_lossy(input);
        for format in formats {
            playlist::parse(format, &text);
        }
    });
}

#[test]
fn podcast_feeds() {
    run("podcast-feeds", &[RSS, ATOM], |input| {
        feed::parse(&String::from_utf8_lossy(input)).ok();
    });
}

#[test]
fn settings_files() {
    run("settings", &[SETTINGS], |input| {
        let Ok(text) = std::str::from_utf8(input) else { return };
        let Ok(doc) = text.parse::<toml_edit::DocumentMut>() else { return };
        toml_edit::de::from_document::<Preferences>(doc.clone()).ok();
        toml_edit::de::from_document::<LocalState>(doc).ok();
    });
}

#[test]
fn update_manifests() {
    run("update-manifests", &[MANIFEST], |input| {
        Manifest::parse(input).ok();
    });
}
