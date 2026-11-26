//! Audio tracks added to what plays, and the J and K keys' delay, in a window playing the mock's
//! clip.

use std::path::PathBuf;

use gpui::TestAppContext;

use super::*;
use crate::engine::backend::Command;
use crate::main_window::now_playing::tests::{draw, open_playing, osd, play_mock, with_tracks};
use crate::main_window::tests::{main_view, open, tap};
use crate::main_window::tests::{toast_levels, toast_messages};
use crate::opening::scratch::scratch_dir;

#[test]
fn the_delay_reads_with_its_sign() {
    assert_eq!(delay_message(Offset::from_millis(0)), "Audio delay: 0 ms");
    assert_eq!(delay_message(Offset::from_millis(-150)), "Audio delay: −150 ms");
    assert_eq!(delay_message(Offset::from_millis(50)), "Audio delay: +50 ms");
}

#[gpui::test]
fn j_and_k_move_the_audio_delay(cx: &mut TestAppContext) {
    let cx = open(cx);
    let playing = play_mock(cx, with_tracks(2));
    tap(cx, "k");
    tap(cx, "k");
    tap(cx, "j");
    assert_eq!(playing.player.read_with(cx, |player, _| player.audio_delay().as_millis()), 50);
    assert_eq!(osd(cx), "Audio delay: +50 ms");
}

#[gpui::test]
fn audio_files_join_what_plays_and_others_are_named(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    let folder = scratch_dir();
    let song = folder.join("dub.mp3");
    std::fs::write(&song, "audio").expect("a scratch file");
    let notes: PathBuf = folder.join("notes.txt");
    playing.mock.take_commands();
    let media = Media::Url("https://e.com/clip".into());
    main_view(cx).update_in(cx, |main, window, cx| {
        main.add_audio_tracks(media, vec![song.clone(), notes], window, cx);
    });
    draw(cx);
    assert!(playing.mock.take_commands().contains(&Command::AddAudio(song)));
    let shown: Vec<String> = toast_messages(cx);
    assert!(shown.iter().any(|text| text == "notes.txt has no audio to add."), "{shown:?}");
    assert!(shown.iter().any(|text| text.starts_with("Added “dub.mp3” as an audio track.")));
}

#[gpui::test]
fn files_with_no_audio_add_nothing(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    playing.mock.take_commands();
    let media = Media::Url("https://e.com/clip".into());
    main_view(cx).update_in(cx, |main, window, cx| {
        main.add_audio_tracks(media, vec![PathBuf::from("/tmp/a.txt")], window, cx);
    });
    draw(cx);
    assert_eq!(playing.mock.take_commands(), [] as [Command; 0]);
    assert_eq!(toast_levels(cx).len(), 1);
}

#[gpui::test]
fn several_files_are_counted(cx: &mut TestAppContext) {
    let cx = open(cx);
    let files = [PathBuf::from("a.mp3"), PathBuf::from("b.mp3")];
    let what = cx.update(|_, cx| added_message(&files, cx));
    assert!(what.starts_with("Added 2 audio tracks."), "{what}");
}

#[gpui::test]
fn audio_for_another_file_opens_it_first(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    let folder = scratch_dir();
    let (video, song) = (folder.join("film.mkv"), folder.join("dub.mp3"));
    std::fs::write(&video, "video").expect("a scratch file");
    std::fs::write(&song, "audio").expect("a scratch file");
    let film = Media::File(video);
    main_view(cx).update_in(cx, |main, window, cx| {
        main.add_audio_tracks(film.clone(), vec![song.clone()], window, cx);
    });
    draw(cx);
    let commands = playing.mock.take_commands();
    let opened =
        commands.iter().any(|c| matches!(c, Command::Open { media, .. } if *media == film));
    assert!(opened, "{commands:?}");
    assert!(commands.contains(&Command::AddAudio(song)), "the audio joins once it plays");
}
