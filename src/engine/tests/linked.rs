//! Player tests for subtitle files linked to a media (S6.4): they load once, after the media's
//! own tracks; the first one the user just linked shows; and an unlinked one leaves the media.

use std::path::PathBuf;

use gpui::TestAppContext;

use super::Rig;
use super::tracks::{movie, selected, track};
use crate::app::media::Media;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::mock::MockMedia;
use crate::engine::player::Linked;
use crate::engine::tracks::{TrackId, TrackKind};

const SRT: &str = "1\n00:00:01,000 --> 00:00:03,000\nHello\n";

/// A folder of its own for the test `test`, holding a video, and `subtitles` in a folder where
/// they aren't found beside it.
fn folder(test: &str, subtitles: &[&str]) -> (Media, Vec<PathBuf>) {
    let dir = std::env::temp_dir().join(format!("rvlc-linked-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Movie.mkv"), b"video").unwrap();
    std::fs::create_dir_all(dir.join("links")).unwrap();
    let paths = subtitles
        .iter()
        .map(|name| {
            let path = dir.join("links").join(name);
            std::fs::write(&path, SRT).unwrap();
            path
        })
        .collect();
    (Media::File(dir.join("Movie.mkv")), paths)
}

/// Opens `media` and lists the movie's own tracks.
fn open(rig: &Rig, cx: &mut TestAppContext, media: &Media) {
    rig.run(cx, |player, cx| player.open(media.clone(), cx));
    rig.emit(cx, BackendEvent::Tracks(movie()));
    rig.mock.take_commands();
}

fn listed(rig: &Rig, cx: &mut TestAppContext) -> Vec<PathBuf> {
    rig.player.read_with(cx, |player, _| {
        player.subtitle_files().iter().map(|file| file.path.clone()).collect()
    })
}

#[gpui::test]
fn linked_files_load_once_without_showing(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    let (media, paths) = folder("once", &["Movie.en.srt", "Movie.he.srt"]);
    open(&rig, cx, &media);
    let first = vec![paths[0].clone()];
    rig.run(cx, |player, cx| player.load_linked_subtitles(&media, first, Linked::OnOpen, cx));
    rig.run(cx, |player, cx| {
        player.load_linked_subtitles(&media, paths.clone(), Linked::OnOpen, cx);
    });
    assert_eq!(listed(&rig, cx), paths);
    let loads: Vec<Command> = paths.iter().cloned().map(Command::LoadSubtitle).collect();
    assert_eq!(rig.mock.take_commands(), loads);
}

#[gpui::test]
fn links_for_other_media_are_dropped(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    let (media, paths) = folder("other", &["Movie.en.srt"]);
    open(&rig, cx, &media);
    let other = Media::File(PathBuf::from("/elsewhere/Other.mkv"));
    rig.run(cx, |player, cx| player.load_linked_subtitles(&other, paths, Linked::OnOpen, cx));
    assert_eq!(listed(&rig, cx), Vec::<PathBuf>::new());
}

#[gpui::test]
fn the_first_file_just_linked_shows(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    let (media, paths) = folder("now", &["Movie.en.srt", "Movie.he.srt"]);
    open(&rig, cx, &media);
    rig.run(cx, |player, cx| player.load_linked_subtitles(&media, paths.clone(), Linked::Now, cx));
    let expected =
        [Command::AddSubtitle(paths[0].clone()), Command::LoadSubtitle(paths[1].clone())];
    assert_eq!(rig.mock.take_commands(), expected);
}

#[gpui::test]
fn an_unlinked_file_leaves_the_media_and_stops_showing(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    let (media, paths) = folder("remove", &["Movie.en.srt"]);
    open(&rig, cx, &media);
    let linked = paths.clone();
    rig.run(cx, |player, cx| player.load_linked_subtitles(&media, linked, Linked::Now, cx));
    let mut added = movie();
    added.all.push(track(7, TrackKind::Subtitle, ""));
    added.subtitle = Some(TrackId(7));
    rig.emit(cx, BackendEvent::Tracks(added.clone()));
    assert_eq!(selected(&rig, cx, TrackKind::Subtitle), Some(TrackId(7)));

    rig.run(cx, |player, cx| player.remove_subtitle_file(&paths[0], cx));
    assert_eq!(listed(&rig, cx), Vec::<PathBuf>::new());
    assert_eq!(selected(&rig, cx, TrackKind::Subtitle), None);
    // The backend still lists the track; the player leaves it out.
    rig.emit(cx, BackendEvent::Tracks(added));
    let tracks = rig.player.read_with(cx, |player, _| player.tracks().clone());
    assert!(tracks.all.iter().all(|listed| listed.id != TrackId(7)));
}

#[gpui::test]
fn linking_a_file_found_beside_the_video_shows_its_track(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::manual());
    let (media, _) = folder("found", &[]);
    let Media::File(video) = &media else { unreachable!("a file") };
    let found = video.with_file_name("Movie.srt");
    std::fs::write(&found, SRT).unwrap();
    open(&rig, cx, &media);
    let mut listed_too = movie();
    listed_too.all.push(track(7, TrackKind::Subtitle, ""));
    rig.emit(cx, BackendEvent::Tracks(listed_too));
    rig.mock.take_commands();

    let linked = vec![found.clone()];
    rig.run(cx, |player, cx| player.load_linked_subtitles(&media, linked, Linked::Now, cx));
    assert_eq!(listed(&rig, cx), [found]);
    assert_eq!(selected(&rig, cx, TrackKind::Subtitle), Some(TrackId(7)));
    let loads = rig.mock.take_commands();
    assert!(!loads.iter().any(|c| matches!(c, Command::AddSubtitle(_) | Command::LoadSubtitle(_))));
}
