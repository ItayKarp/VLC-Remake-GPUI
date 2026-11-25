//! The track popover drawn on a window playing on the mock backend: its rows pick tracks,
//! devices and subtitle files, and "+" adds subtitle files.

use gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};

use crate::engine::audio::AudioDevice;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::{MockHandle, MockMedia};
use crate::engine::tracks::{Track, TrackId, TrackKind};
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open, toast_messages};
use crate::main_window::video_menu::tests::support::{emit, movie, player, playing};
use crate::opening::scratch::scratch_dir;
use crate::settings;

/// The window playing the movie, its popover open on the tab of `kind` and drawn.
fn popover(cx: &mut TestAppContext, kind: TrackKind) -> (&mut VisualTestContext, MockHandle) {
    let (cx, mock) = playing(cx, MockMedia { tracks: movie(), ..MockMedia::default() });
    mock.take_commands();
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.open_track_popover(kind, window, cx));
    draw(cx);
    (cx, mock)
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// Clicks the row drawn with debug id `id`, and draws the result.
fn click(cx: &mut VisualTestContext, id: &'static str) {
    let at = cx.debug_bounds(id).unwrap_or_else(|| panic!("{id} isn't drawn")).center();
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    draw(cx);
}

fn selected(cx: &mut VisualTestContext, kind: TrackKind) -> Option<TrackId> {
    let player = player(cx);
    player.read_with(cx, |player, _| player.tracks().selected(kind))
}

fn output_device(cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| settings::get(cx).local.audio.output_device.clone())
}

#[gpui::test]
fn the_audio_tab_picks_a_track_and_a_device(cx: &mut TestAppContext) {
    let (cx, mock) = popover(cx, TrackKind::Audio);
    assert!(cx.debug_bounds("track-embedded-2").is_some(), "None and the two tracks");
    assert!(cx.debug_bounds("track-embedded-3").is_none());
    click(cx, "track-embedded-2");
    assert_eq!(selected(cx, TrackKind::Audio), Some(TrackId(3)));
    click(cx, "track-embedded-0");
    assert_eq!(selected(cx, TrackKind::Audio), None);

    let speakers = AudioDevice { id: "speakers".into(), name: "Desk Speakers".into() };
    emit(cx, &mock, BackendEvent::AudioDevices(vec![speakers]));
    draw(cx);
    click(cx, "track-device-1");
    assert_eq!(output_device(cx), "speakers");
    click(cx, "track-device-0");
    assert_eq!(output_device(cx), "");
}

#[gpui::test]
fn looking_for_devices_again_asks_the_backend(cx: &mut TestAppContext) {
    let (cx, mock) = popover(cx, TrackKind::Audio);
    let main = main_view(cx);
    main.update(cx, |main, cx| main.refresh_devices(cx));
    assert!(mock.take_commands().contains(&Command::RefreshAudioDevices));
}

#[gpui::test]
fn the_subtitles_tab_lists_files_apart_from_embedded_tracks(cx: &mut TestAppContext) {
    let (cx, mock) = popover(cx, TrackKind::Subtitle);
    assert!(cx.debug_bounds("subtitle-file-0").is_none(), "no files yet");
    click(cx, "track-embedded-1");
    assert_eq!(selected(cx, TrackKind::Subtitle), Some(TrackId(4)));

    let file = scratch_dir().join("Clip.fr.srt");
    let main = main_view(cx);
    main.update(cx, |main, cx| main.attach_subtitles(std::slice::from_ref(&file), cx));
    draw(cx);
    assert!(cx.debug_bounds("subtitle-file-0").is_some(), "a file waiting for its track");
    let mut added = movie();
    added.all.push(Track::new(TrackId(7), TrackKind::Subtitle));
    emit(cx, &mock, BackendEvent::Tracks(added));
    draw(cx);
    assert!(cx.debug_bounds("track-embedded-3").is_none(), "the file's track isn't embedded");
    click(cx, "subtitle-file-0");
    assert_eq!(selected(cx, TrackKind::Subtitle), Some(TrackId(7)));
}

#[gpui::test]
fn adding_subtitle_files_turns_away_other_files(cx: &mut TestAppContext) {
    let (cx, mock) = popover(cx, TrackKind::Subtitle);
    let folder = scratch_dir();
    let (subtitle, notes) = (folder.join("Clip.de.srt"), folder.join("notes.txt"));
    let main = main_view(cx);
    main.update_in(cx, |_, window, cx| MainWindow::prompt_for_subtitles(window, cx));
    cx.simulate_path_prompt_response(|_| Some(vec![subtitle.clone(), notes]));
    cx.run_until_parked();
    assert_eq!(toast_messages(cx), ["notes.txt isn't a subtitle file."]);
    assert!(mock.take_commands().contains(&Command::AddSubtitle(subtitle)));
}

#[gpui::test]
fn adding_subtitles_with_nothing_playing_says_to_open_a_video(cx: &mut TestAppContext) {
    let cx = open(cx);
    let main = main_view(cx);
    main.update(cx, |main, cx| main.attach_subtitles(&["a.srt".into()], cx));
    assert_eq!(toast_messages(cx), ["Open a video first, then add subtitles."]);
}

#[gpui::test]
fn the_video_tab_shows_with_two_video_tracks(cx: &mut TestAppContext) {
    let (cx, mock) = popover(cx, TrackKind::Video);
    assert!(cx.debug_bounds("track-device-0").is_some(), "one video track: the Audio tab");
    let mut two = movie();
    two.all.push(Track::new(TrackId(9), TrackKind::Video));
    emit(cx, &mock, BackendEvent::Tracks(two));
    draw(cx);
    assert!(cx.debug_bounds("track-device-0").is_none(), "the Video tab");
    click(cx, "track-embedded-2");
    assert_eq!(selected(cx, TrackKind::Video), Some(TrackId(9)));
}

#[gpui::test]
fn an_unplugged_device_falls_back_to_the_system_default(cx: &mut TestAppContext) {
    let (cx, mock) = popover(cx, TrackKind::Audio);
    cx.update(|_, cx| settings::update(cx, |s| s.local.audio.output_device = "gone".into()));
    let speakers = AudioDevice { id: "speakers".into(), name: "Desk Speakers".into() };
    emit(cx, &mock, BackendEvent::AudioDevices(vec![speakers]));
    draw(cx);
    let player = player(cx);
    let (preferred, active) = player.read_with(cx, |player, _| {
        (player.preferred_audio_device().clone(), player.active_audio_device().clone())
    });
    assert_ne!(preferred, active, "the note shows");
    assert!(cx.debug_bounds("track-device-0").is_some());
}
