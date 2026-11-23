//! Player tests for subtitle files the app draws itself (S6.2): the backend shows none while
//! the app draws a file, styled files follow Embedded Sub Format, and cues follow the clock;
//! and the subtitle timeline's lines (S6.6).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gpui::TestAppContext;

use super::Rig;
use super::tracks::{movie, open_with, selected, track};
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::MockMedia;
use crate::engine::player::CueSpan;
use crate::engine::state::PlaybackState;
use crate::engine::subtitle_sync::SubtitleSync;
use crate::engine::tracks::{TrackId, TrackKind};
use crate::engine::values::Offset;

const SRT: &str = "1\n00:00:01,000 --> 00:00:03,000\nHello\n";
const ASS: &str = "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, \
                   MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hi\n";

/// Writes `contents` as `name` in a folder of its own for the test `test`.
fn file(test: &str, name: &str, contents: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rvlc-drawn-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, contents).unwrap();
    path
}

/// Opens the movie playing, adds the subtitle file at `path`, and lists it as track 7, shown
/// as the backend shows an added file.
fn add_shown(rig: &Rig, cx: &mut TestAppContext, path: &Path) {
    open_with(rig, cx, movie());
    rig.emit(cx, BackendEvent::State(PlaybackState::Playing));
    rig.run(cx, |player, cx| assert!(player.add_subtitle(path, cx)));
    let mut added = movie();
    added.all.push(track(7, TrackKind::Subtitle, ""));
    added.subtitle = Some(TrackId(7));
    rig.mock.take_commands();
    rig.emit(cx, BackendEvent::Tracks(added));
}

fn shown_text(rig: &Rig, cx: &mut TestAppContext) -> Option<Vec<String>> {
    rig.player.read_with(cx, |player, _| {
        let cues = player.drawn_subtitles(Instant::now())?;
        Some(cues.into_iter().map(crate::engine::cues::Cue::text).collect())
    })
}

#[gpui::test]
fn the_app_draws_a_text_file_and_the_backend_shows_none(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    add_shown(&rig, cx, &file("srt", "Movie.srt", SRT));
    let deselect = Command::SelectTrack(TrackKind::Subtitle, None);
    assert_eq!(rig.mock.take_commands(), [deselect]);
    assert_eq!(selected(&rig, cx, TrackKind::Subtitle), Some(TrackId(7)));
    rig.emit(cx, BackendEvent::Position(Duration::from_millis(1_500)));
    assert_eq!(shown_text(&rig, cx), Some(vec!["Hello".to_owned()]));

    // Picking the media's own subtitles hands drawing back to the backend.
    rig.run(cx, |player, cx| player.select_track(TrackKind::Subtitle, Some(TrackId(3)), cx));
    let embedded = Command::SelectTrack(TrackKind::Subtitle, Some(TrackId(3)));
    assert_eq!(rig.mock.take_commands(), [embedded]);
    assert_eq!(shown_text(&rig, cx), None);
}

#[gpui::test]
fn styled_files_follow_embedded_sub_format(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    add_shown(&rig, cx, &file("ass", "Movie.ass", ASS));
    assert_eq!(rig.mock.take_commands(), [], "the backend draws it in its own style");
    assert!(rig.player.read_with(cx, |player, _| player.backend_draws_subtitle()));

    rig.run(cx, |player, cx| player.set_file_styles(false, cx));
    let deselect = Command::SelectTrack(TrackKind::Subtitle, None);
    assert_eq!(rig.mock.take_commands(), [deselect]);
    assert!(!rig.player.read_with(cx, |player, _| player.backend_draws_subtitle()));

    rig.run(cx, |player, cx| player.set_file_styles(true, cx));
    let shown = Command::SelectTrack(TrackKind::Subtitle, Some(TrackId(7)));
    assert_eq!(rig.mock.take_commands(), [shown]);
}

#[gpui::test]
fn the_timeline_gets_the_lines_with_their_sync(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    add_shown(&rig, cx, &file("timeline", "Movie.srt", SRT));
    let secs = Duration::from_secs;
    let sync = SubtitleSync::default().with_delay(Offset::from_millis(2_000)).with_speed(2.);
    rig.run(cx, |player, cx| assert!(player.set_subtitle_sync(sync, cx).is_some()));
    let spans = rig.player.read_with(cx, |player, _| player.subtitle_spans(secs(0), secs(10)));
    let delayed =
        CueSpan { start: Duration::from_millis(2_500), end: Duration::from_millis(3_500) };
    assert_eq!(spans, Some(vec![delayed]));

    // The media's own subtitles have no known times.
    rig.run(cx, |player, cx| player.select_track(TrackKind::Subtitle, Some(TrackId(3)), cx));
    assert!(!rig.player.read_with(cx, |player, _| player.knows_subtitle_timings()));
}
