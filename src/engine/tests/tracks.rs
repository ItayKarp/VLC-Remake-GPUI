//! Player track tests (S1.6): picking by preferred language, forced subtitles, cycling, and the
//! user's picks surviving late track changes.

use gpui::TestAppContext;

use super::Rig;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::language::Language;
use crate::engine::mock::MockMedia;
use crate::engine::tracks::auto::{SubtitleMode, TrackPreferences};
use crate::engine::tracks::{Cycled, Track, TrackId, TrackKind, Tracks};

fn track(id: i32, kind: TrackKind, language: &str) -> Track {
    Track { language: Language::parse(language), ..Track::new(TrackId(id), kind) }
}

/// Japanese and English audio with English playing; English, forced English and French
/// subtitles, none shown.
fn movie() -> Tracks {
    Tracks {
        all: vec![
            track(1, TrackKind::Audio, "jpn"),
            track(2, TrackKind::Audio, "eng"),
            track(3, TrackKind::Subtitle, "eng"),
            Track { forced: true, ..track(4, TrackKind::Subtitle, "eng") },
            track(5, TrackKind::Subtitle, "fre"),
        ],
        audio: Some(TrackId(2)),
        ..Tracks::default()
    }
}

fn wanting(rig: &Rig, cx: &mut TestAppContext, audio: &str, subtitles: SubtitleMode) {
    let preferences = TrackPreferences {
        audio_languages: Language::parse(audio).into_iter().collect(),
        subtitle_languages: Vec::new(),
        subtitles,
        ..TrackPreferences::default()
    };
    rig.run(cx, |player, _| player.set_track_preferences(preferences));
}

/// Opens with the mock silent, takes the open's commands, then reports `tracks`.
fn open_with(rig: &Rig, cx: &mut TestAppContext, tracks: Tracks) {
    rig.open(cx);
    rig.mock.take_commands();
    rig.emit(cx, BackendEvent::Tracks(tracks));
}

fn selected(rig: &Rig, cx: &mut TestAppContext, kind: TrackKind) -> Option<TrackId> {
    rig.player.read_with(cx, |player, _| player.tracks().selected(kind))
}

#[gpui::test]
fn new_media_plays_the_preferred_audio_with_its_forced_subtitles(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    wanting(&rig, cx, "ja", SubtitleMode::ForcedOnly);
    let mut english = movie();
    english.subtitle = Some(TrackId(3));
    open_with(&rig, cx, english);
    // Japanese has no forced track, so the shown English subtitles go off.
    let picks = [
        Command::SelectTrack(TrackKind::Audio, Some(TrackId(1))),
        Command::SelectTrack(TrackKind::Subtitle, None),
    ];
    assert_eq!(rig.mock.take_commands(), picks);
    assert_eq!(selected(&rig, cx, TrackKind::Audio), Some(TrackId(1)));

    wanting(&rig, cx, "en", SubtitleMode::Show);
    open_with(&rig, cx, movie());
    let forced = [Command::SelectTrack(TrackKind::Subtitle, Some(TrackId(4)))];
    assert_eq!(rig.mock.take_commands(), forced);
}

#[gpui::test]
fn the_users_pick_survives_late_tracks(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    wanting(&rig, cx, "ja", SubtitleMode::Show);
    open_with(&rig, cx, movie());
    rig.run(cx, |player, cx| player.select_track(TrackKind::Audio, Some(TrackId(2)), cx));
    rig.mock.take_commands();
    let mut late = movie();
    late.all.push(track(6, TrackKind::Audio, "jpn"));
    rig.emit(cx, BackendEvent::Tracks(late));
    // Forced subtitles follow the audio the user chose; the audio itself stays.
    let forced = [Command::SelectTrack(TrackKind::Subtitle, Some(TrackId(4)))];
    assert_eq!(rig.mock.take_commands(), forced);
    assert_eq!(selected(&rig, cx, TrackKind::Audio), Some(TrackId(2)));
}

#[gpui::test]
fn cycling_subtitles_passes_through_off(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    wanting(&rig, cx, "", SubtitleMode::Show);
    let mut french = movie();
    french.subtitle = Some(TrackId(5));
    open_with(&rig, cx, french);
    let cycle = |cx: &mut TestAppContext| {
        rig.player.update(cx, |player, cx| player.cycle_track(TrackKind::Subtitle, cx))
    };
    assert_eq!(cycle(cx), Cycled::Off);
    assert_eq!(cycle(cx), Cycled::Track(TrackId(3)));
    let sent = [
        Command::SelectTrack(TrackKind::Subtitle, None),
        Command::SelectTrack(TrackKind::Subtitle, Some(TrackId(3))),
    ];
    assert_eq!(rig.mock.take_commands(), sent);
    let cycle_video = rig.player.update(cx, |player, cx| player.cycle_track(TrackKind::Video, cx));
    assert_eq!(cycle_video, Cycled::NoTracks);
}

#[gpui::test]
fn an_added_subtitle_file_stays_shown(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    wanting(&rig, cx, "", SubtitleMode::ForcedOnly);
    open_with(&rig, cx, movie());
    rig.emit(cx, BackendEvent::State(crate::engine::state::PlaybackState::Playing));
    let path = std::path::Path::new("/tmp/movie.srt");
    assert!(rig.player.update(cx, |player, cx| player.add_subtitle(path, cx)));
    rig.mock.take_commands();
    let mut added = movie();
    added.all.push(track(7, TrackKind::Subtitle, "und"));
    added.subtitle = Some(TrackId(7));
    rig.emit(cx, BackendEvent::Tracks(added));
    assert_eq!(rig.mock.take_commands(), []);
    let files = rig.player.read_with(cx, |player, _| player.subtitle_files().to_vec());
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].track, Some(TrackId(7)));
}

#[gpui::test]
fn an_added_audio_file_plays_and_stays_chosen(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    let path = std::path::Path::new("/tmp/dub.mka");
    assert!(!rig.player.update(cx, |player, cx| player.add_audio(path, cx)), "nothing is open");
    open_with(&rig, cx, movie());
    rig.emit(cx, BackendEvent::State(crate::engine::state::PlaybackState::Playing));
    rig.mock.take_commands();
    assert!(rig.player.update(cx, |player, cx| player.add_audio(path, cx)));
    assert_eq!(rig.mock.take_commands(), [Command::AddAudio(path.to_path_buf())]);
    let mut added = movie();
    added.all.push(track(8, TrackKind::Audio, "und"));
    added.audio = Some(TrackId(8));
    rig.emit(cx, BackendEvent::Tracks(added));
    // Auto-selection leaves the added track playing.
    assert_eq!(rig.mock.take_commands(), []);
}
