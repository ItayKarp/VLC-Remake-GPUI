//! Video ▸, Audio ▸ and Subtitle ▸: the tracks, the framing held for the session, the audio
//! settings and the subtitle file's encoding and delay.

use gpui::{TestAppContext, VisualTestContext};

use super::support::{
    checked, choose, emit, enabled, find, labels, movie, player, playing, reason, submenu,
};
use super::{items, pick};
use crate::engine::audio::visualizer::Visualizer;
use crate::engine::audio::{AudioDevice, SpeakerSetup};
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::{MockHandle, MockMedia};
use crate::engine::state::PlaybackState;
use crate::engine::tracks::{Track, TrackId, TrackKind};
use crate::engine::values::Offset;
use crate::main_window::tests::main_view;
use crate::settings;

/// The window playing the movie's tracks.
fn movie_playing(cx: &mut TestAppContext) -> (&mut VisualTestContext, MockHandle) {
    let (cx, mock) = playing(cx, MockMedia { tracks: movie(), ..MockMedia::default() });
    mock.take_commands();
    (cx, mock)
}

/// The window playing the movie's tracks on a mock that answers nothing by itself, so the
/// test says which tracks there are.
fn movie_playing_manually(cx: &mut TestAppContext) -> (&mut VisualTestContext, MockHandle) {
    let (cx, mock) = playing(cx, MockMedia::manual());
    for event in [
        BackendEvent::Duration(Some(std::time::Duration::from_secs(60))),
        BackendEvent::Seekable(true),
        BackendEvent::Tracks(movie()),
        BackendEvent::State(PlaybackState::Playing),
    ] {
        emit(cx, &mock, event);
    }
    mock.take_commands();
    (cx, mock)
}

fn selected(cx: &mut VisualTestContext, kind: TrackKind) -> Option<TrackId> {
    let player = player(cx);
    player.read_with(cx, |player, _| player.tracks().selected(kind))
}

#[gpui::test]
fn the_track_menus_check_the_playing_track_and_switch(cx: &mut TestAppContext) {
    let (cx, _mock) = movie_playing(cx);
    let menu = items(cx);
    let audio = submenu(find(&menu, &["Audio", "Audio Track"]));
    assert_eq!(labels(audio).len(), 3, "None and two tracks: {:?}", labels(audio));
    assert_eq!(checked(audio).len(), 1);
    let japanese = labels(audio)[2].clone();
    choose(cx, find(&menu, &["Audio", "Audio Track", &japanese]));
    assert_eq!(selected(cx, TrackKind::Audio), Some(TrackId(3)));

    let subtitles = items(cx);
    assert_eq!(checked(submenu(find(&subtitles, &["Subtitle", "Subtitle Track"]))), ["None"]);
    let english = labels(submenu(find(&subtitles, &["Subtitle", "Subtitle Track"])))[1].clone();
    choose(cx, find(&subtitles, &["Subtitle", "Subtitle Track", &english]));
    assert_eq!(selected(cx, TrackKind::Subtitle), Some(TrackId(4)));

    pick(cx, &["Video", "Video Track", "None"]);
    assert_eq!(selected(cx, TrackKind::Video), None);
}

#[gpui::test]
fn media_without_tracks_of_a_kind_says_so(cx: &mut TestAppContext) {
    let (cx, _mock) = playing(cx, MockMedia::default());
    let menu = items(cx);
    let video = find(&menu, &["Video", "Video Track"]);
    assert!(!enabled(video));
    assert_eq!(reason(video).as_deref(), Some("No video tracks"));
    assert_eq!(
        reason(find(&menu, &["Subtitle", "Subtitle Track"])).as_deref(),
        Some("No subtitles")
    );
    let snapshot = find(&menu, &["Video", "Take Snapshot"]);
    assert!(!enabled(snapshot));
    assert!(reason(snapshot).is_some());
}

#[gpui::test]
fn the_framing_menus_hold_a_choice_for_the_session(cx: &mut TestAppContext) {
    let (cx, _mock) = movie_playing(cx);
    let menu = items(cx);
    let zoom = labels(submenu(find(&menu, &["Video", "Zoom"])));
    assert!(zoom.len() > 1, "{zoom:?}");
    assert_eq!(checked(submenu(find(&menu, &["Video", "Zoom"]))).len(), 1);
    let other = zoom
        .iter()
        .find(|label| !checked(submenu(find(&menu, &["Video", "Zoom"]))).contains(label));
    let other = other.expect("an unchecked zoom").clone();
    choose(cx, find(&menu, &["Video", "Zoom", &other]));
    assert_eq!(checked(submenu(find(&items(cx), &["Video", "Zoom"]))), [other]);
}

#[gpui::test]
fn the_deinterlacing_method_waits_for_deinterlacing(cx: &mut TestAppContext) {
    let (cx, _mock) = movie_playing(cx);
    let choices = labels(submenu(find(&items(cx), &["Video", "Deinterlace"])));
    for choice in choices {
        pick(cx, &["Video", "Deinterlace", &choice]);
        let menu = items(cx);
        let method = find(&menu, &["Video", "Deinterlacing Method"]);
        let why = (choice == "Off").then(|| "Deinterlacing is off".to_owned());
        assert_eq!(reason(method), why, "{choice}");
    }
}

#[gpui::test]
fn fullscreen_and_picture_in_picture_are_offered(cx: &mut TestAppContext) {
    let (cx, _mock) = movie_playing(cx);
    assert!(enabled(find(&items(cx), &["Video", "Picture in Picture"])));
    pick(cx, &["Video", "Fullscreen"]);
    assert!(cx.update(|window, _| window.is_fullscreen()));
    assert!(labels(submenu(find(&items(cx), &["Video"]))).contains(&"Exit Fullscreen".into()));
}

#[gpui::test]
fn audio_sets_the_device_speakers_visualizer_and_mute(cx: &mut TestAppContext) {
    let (cx, mock) = movie_playing(cx);
    let speakers = AudioDevice { id: "speakers".into(), name: "Desk Speakers".into() };
    emit(cx, &mock, BackendEvent::AudioDevices(vec![speakers]));
    let devices = items(cx);
    let devices = submenu(find(&devices, &["Audio", "Audio Device"]));
    assert_eq!(labels(devices), ["System default", "Desk Speakers"]);
    assert_eq!(checked(devices), ["System default"]);
    pick(cx, &["Audio", "Audio Device", "Desk Speakers"]);
    let device = cx.update(|_, cx| settings::get(cx).local.audio.output_device.clone());
    assert_eq!(device, "speakers");

    pick(cx, &["Audio", "Stereo Mode", "Stereo"]);
    let setup = cx.update(|_, cx| settings::get(cx).prefs.audio.speaker_setup);
    assert_eq!(setup, SpeakerSetup::Stereo);
    pick(cx, &["Audio", "Visualizer", Visualizer::Scope.label()]);
    let shown = cx.update(|_, cx| settings::get(cx).prefs.audio.visualizer);
    assert_eq!(shown, Visualizer::Scope);

    pick(cx, &["Audio", "Mute"]);
    let player = player(cx);
    assert!(player.read_with(cx, |player, _| player.is_muted()));
    assert_eq!(
        checked(submenu(find(&items(cx), &["Audio"]))).last().map(String::as_str),
        Some("Mute")
    );
    pick(cx, &["Audio", "Increase Volume"]);
    pick(cx, &["Audio", "Decrease Volume"]);
}

#[gpui::test]
fn the_subtitle_delay_steps_and_resets(cx: &mut TestAppContext) {
    let (cx, _mock) = movie_playing(cx);
    let reset = items(cx);
    let reset = find(&reset, &["Subtitle", "Reset Delay"]);
    assert!(!enabled(reset));
    assert_eq!(reason(reset).as_deref(), Some("No subtitles showing"));
    let english = labels(submenu(find(&items(cx), &["Subtitle", "Subtitle Track"])))[1].clone();
    pick(cx, &["Subtitle", "Subtitle Track", &english]);
    pick(cx, &["Subtitle", "Increase Delay"]);
    let player = player(cx);
    let delay = |cx: &mut VisualTestContext| {
        player.read_with(cx, |player, _| player.subtitle_sync().delay())
    };
    assert_ne!(delay(cx), Offset::default());
    pick(cx, &["Subtitle", "Reset Delay"]);
    assert_eq!(delay(cx), Offset::default());
}

#[gpui::test]
fn a_text_subtitle_file_can_be_read_in_another_encoding(cx: &mut TestAppContext) {
    let (cx, mock) = movie_playing_manually(cx);
    let encoding = items(cx);
    let encoding = find(&encoding, &["Subtitle", "Text Encoding"]);
    assert!(!enabled(encoding));
    assert_eq!(reason(encoding).as_deref(), Some("The subtitles showing aren't from a text file"));
    // A file that can't be read loads as it is, so reading it again writes no converted copy.
    let file = crate::opening::scratch::scratch_dir().join("Clip.en.srt");
    let main = main_view(cx);
    main.update(cx, |main, cx| main.attach_subtitles(std::slice::from_ref(&file), cx));
    cx.run_until_parked();
    let mut added = movie();
    added.all.push(Track::new(TrackId(7), TrackKind::Subtitle));
    added.subtitle = Some(TrackId(7));
    emit(cx, &mock, BackendEvent::Tracks(added));
    mock.take_commands();

    let menu = items(cx);
    assert!(
        labels(submenu(find(&menu, &["Subtitle", "Subtitle Track"])))
            .contains(&"Clip.en.srt".into())
    );
    let encodings = submenu(find(&menu, &["Subtitle", "Text Encoding"]));
    assert!(checked(encodings)[0].starts_with("Automatic"), "{:?}", checked(encodings));
    let other = labels(encodings)[1].clone();
    choose(cx, find(&menu, &["Subtitle", "Text Encoding", &other]));
    let reread = mock.take_commands();
    assert!(reread.contains(&Command::AddSubtitle(file)), "{reread:?}");
}
