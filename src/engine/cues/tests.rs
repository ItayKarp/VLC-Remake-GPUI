use std::path::Path;
use std::time::Duration;

use super::*;

fn read(name: &str, text: &str) -> CueTrack {
    parse(Path::new(name), text).expect("the file reads")
}

fn texts_at(track: &CueTrack, millis: u64) -> Vec<String> {
    track.at(Duration::from_millis(millis)).map(Cue::text).collect()
}

#[test]
fn srt_reads_leniently_and_sorts() {
    let srt = "\u{FEFF}2\r\n00:00:05,000 --> 00:00:06,000\r\nSecond\r\n\r\n\
               1\n00:00:01.000 --> 00:00:03,000\nFirst <i>line</i>\nand more\n\
               3\n00:00:07,000 --> 00:00:08,000\n\u{FEFF}Third\n\n\
               4\nnot a time --> either\nBroken\n";
    let track = read("Movie.srt", srt);
    assert_eq!(track.len(), 3);
    assert_eq!(texts_at(&track, 1_500), ["First line\nand more"]);
    assert_eq!(texts_at(&track, 5_500), ["Second"]);
    assert_eq!(texts_at(&track, 7_500), ["Third"]);
    assert_eq!(texts_at(&track, 4_000), Vec::<String>::new());
    assert!(!track.is_styled());
}

#[test]
fn overlapping_cues_show_together_earliest_first() {
    let srt = "1\n00:00:01,000 --> 00:00:10,000\nLong\n\n\
               2\n00:00:02,000 --> 00:00:03,000\nShort\n";
    let track = read("a.srt", srt);
    assert_eq!(texts_at(&track, 2_500), ["Long", "Short"]);
    assert_eq!(texts_at(&track, 9_000), ["Long"]);
    assert_eq!(texts_at(&track, 10_000), Vec::<String>::new());
}

#[test]
fn a_duration_factor_stretches_lines_up_to_the_next_one() {
    let srt = "1\n00:00:01,000 --> 00:00:02,000\nOne\n\n\
               2\n00:00:04,000 --> 00:00:05,000\nTwo\n";
    let track = read("a.srt", srt);
    let at = |millis, factor| -> Vec<String> {
        track
            .at_stretched(Duration::from_millis(millis), factor)
            .into_iter()
            .map(Cue::text)
            .collect()
    };
    assert_eq!(at(2_500, 0.), Vec::<String>::new());
    assert_eq!(at(2_500, 2.), ["One"]);
    // Ten times as long would reach 11 s; it stops where Two starts.
    assert_eq!(at(3_900, 10.), ["One"]);
    assert_eq!(at(4_500, 10.), ["Two"]);
    assert_eq!(at(13_900, 10.), ["Two"]);
    assert_eq!(at(1_600, 0.5), Vec::<String>::new());
}

#[test]
fn webvtt_skips_its_header_and_notes_and_reads_line_settings() {
    let vtt = "WEBVTT\n\nNOTE a comment\n\nintro\n00:01.000 --> 00:02.000 line:0\n\
               <v Roger>Top</v>\n\n00:03.000 --> 00:04.000 align:start\nBottom\n";
    let track = read("a.vtt", vtt);
    let top: Vec<&Cue> = track.at(Duration::from_millis(1_500)).collect();
    assert_eq!(top[0].text(), "Top");
    assert_eq!(top[0].placement, Placement::Top);
    let bottom: Vec<&Cue> = track.at(Duration::from_millis(3_500)).collect();
    assert_eq!(bottom[0].placement, Placement::Bottom);
}

#[test]
fn ass_dialogue_follows_its_format_and_styles() {
    let ass = "[Script Info]\nTitle: x\n\n[V4+ Styles]\n\
               Format: Name, Fontname, Alignment\nStyle: Sign,Arial,8\nStyle: Default,Arial,2\n\n\
               [Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, \
               Effect, Text\n\
               Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello, {\\i1}world{\\i0}\\Nagain\n\
               Dialogue: 0,0:00:01.00,0:00:02.00,Sign,,0,0,0,,EXIT\n\
               Comment: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,hidden\n\
               Dialogue: 0,0:00:03.00,0:00:04.00,Default,,0,0,0,,{\\p1}m 0 0 l 10 10{\\p0}\n";
    let track = read("a.ass", ass);
    assert!(track.is_styled());
    let cues: Vec<&Cue> = track.at(Duration::from_millis(1_500)).collect();
    assert_eq!(cues[0].text(), "Hello, world\nagain");
    assert_eq!(cues[0].spans[1].emphasis, Emphasis::ITALIC);
    assert_eq!(cues[0].placement, Placement::Bottom);
    assert_eq!((cues[1].text().as_str(), cues[1].placement), ("EXIT", Placement::Top));
    assert_eq!(track.len(), 2, "a comment and a drawing are no cues");
}

#[test]
fn microdvd_needs_its_frame_rate() {
    let with_rate = "{1}{1}25\n{25}{50}{y:i}One|two\n{75}{100}{c:$0000FF}Red\n";
    let track = read("a.sub", with_rate);
    let cues: Vec<&Cue> = track.at(Duration::from_millis(1_500)).collect();
    assert_eq!(cues[0].text(), "One\ntwo");
    assert_eq!(cues[0].spans[0].emphasis, Emphasis::ITALIC);
    let red: Vec<&Cue> = track.at(Duration::from_millis(3_500)).collect();
    assert_eq!(red[0].spans[0].color, Some(0xFF_0000));
    assert_eq!(parse(Path::new("a.sub"), "{25}{50}No rate\n"), None);
}

#[test]
fn formats_libvlc_draws_and_empty_files_are_none() {
    assert_eq!(parse(Path::new("a.ttml"), "<tt/>"), None);
    assert_eq!(parse(Path::new("a.idx"), "# VobSub"), None);
    assert_eq!(parse(Path::new("a.srt"), "no cues here"), None);
}
