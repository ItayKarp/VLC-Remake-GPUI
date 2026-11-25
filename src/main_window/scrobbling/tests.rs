//! When a play is handed over to be scrobbled, from the player's events alone, and which media
//! and tags make a track.

use std::path::PathBuf;

use gpui::TestAppContext;

use super::*;
use crate::settings;

const LENGTH: Option<Duration> = Some(Duration::from_secs(100));

fn play(artist: &str, title: &str) -> Play {
    Play { artist: artist.into(), title: title.into(), album: None, duration_s: None, started_s: 0 }
}

/// Starts playing `track` of `length`.
fn start(scrobbling: &mut Scrobbling, track: &Play, length: Option<Duration>) {
    let opening = PlayerEvent::StateChanged(PlaybackState::Opening);
    assert_eq!(scrobbling.follow(&opening, || None, length), None);
    let playing = PlayerEvent::StateChanged(PlaybackState::Playing);
    assert_eq!(scrobbling.follow(&playing, || Some(track.clone()), length), None);
}

/// Plays on from `from` to `to` seconds, a second at a time, and answers what was handed over.
fn listen(scrobbling: &mut Scrobbling, from: u64, to: u64, length: Option<Duration>) -> Vec<Play> {
    let at = |seconds| PlayerEvent::PositionChanged(Duration::from_secs(seconds));
    (from..=to).filter_map(|seconds| scrobbling.follow(&at(seconds), || None, length)).collect()
}

#[test]
fn a_track_is_handed_over_once_half_of_it_is_heard() {
    let mut scrobbling = Scrobbling::default();
    let track = play("Artist", "Song");
    start(&mut scrobbling, &track, LENGTH);
    assert_eq!(listen(&mut scrobbling, 0, 49, LENGTH), []);
    let sent = listen(&mut scrobbling, 50, 100, LENGTH);
    assert_eq!(sent, [Play { duration_s: Some(100), ..track.clone() }], "once per play");
    // Playing it again is another play.
    start(&mut scrobbling, &track, LENGTH);
    assert_eq!(listen(&mut scrobbling, 0, 60, LENGTH).len(), 1);
}

#[test]
fn seeking_and_pausing_dont_count_as_heard() {
    let mut scrobbling = Scrobbling::default();
    start(&mut scrobbling, &play("Artist", "Song"), LENGTH);
    assert_eq!(listen(&mut scrobbling, 0, 10, LENGTH), []);
    // A jump to the end is a seek, not listening.
    assert_eq!(listen(&mut scrobbling, 90, 99, LENGTH), []);
    let paused = PlayerEvent::StateChanged(PlaybackState::Paused);
    assert_eq!(scrobbling.follow(&paused, || None, LENGTH), None);
    assert_eq!(listen(&mut scrobbling, 99, 99, LENGTH), []);
}

#[test]
fn media_without_a_track_is_never_handed_over() {
    let mut scrobbling = Scrobbling::default();
    let playing = PlayerEvent::StateChanged(PlaybackState::Playing);
    assert_eq!(scrobbling.follow(&playing, || None, LENGTH), None);
    assert_eq!(listen(&mut scrobbling, 0, 100, LENGTH), []);
    // Its tags arrive late: from then on it is a track.
    let tagged = || Some(play("Artist", "Song"));
    assert_eq!(scrobbling.follow(&PlayerEvent::MetadataChanged, tagged, LENGTH), None);
    assert_eq!(listen(&mut scrobbling, 100, 100, LENGTH).len(), 1);
}

#[test]
fn a_stream_naming_a_new_song_starts_a_new_track() {
    let mut scrobbling = Scrobbling::default();
    start(&mut scrobbling, &play("Station", "First"), None);
    let retag = |scrobbling: &mut Scrobbling, track: Play| {
        scrobbling.follow(&PlayerEvent::MetadataChanged, || Some(track), None)
    };
    assert_eq!(listen(&mut scrobbling, 0, 120, None), []);
    // The same tags again keep what was heard.
    assert_eq!(retag(&mut scrobbling, play("Station", "First")), None);
    let first = listen(&mut scrobbling, 121, 240, None);
    assert_eq!(first.iter().map(|play| play.title.as_str()).collect::<Vec<_>>(), ["First"]);
    assert_eq!(retag(&mut scrobbling, play("Station", "Second")), None);
    assert_eq!(listen(&mut scrobbling, 241, 300, None), [], "the new song starts from nothing");
    let second = listen(&mut scrobbling, 301, 490, None);
    assert_eq!(second.iter().map(|play| play.title.as_str()).collect::<Vec<_>>(), ["Second"]);
}

fn tags(artist: &str, title: &str, album: &str) -> Metadata {
    let text = |text: &str| Some(text.to_owned());
    Metadata { title: text(title), artist: text(artist), album: text(album) }
}

#[gpui::test]
fn a_track_needs_submitting_on_an_artist_and_a_title(cx: &mut TestAppContext) {
    cx.update(settings::init_in_memory);
    let song = Media::File(PathBuf::from("/music/song.mp3"));
    let tagged = tags(" Artist ", "Song", " ");
    cx.update(|cx| assert_eq!(track(&song, &tagged, cx), None, "submitting is off"));
    cx.update(|cx| settings::update(cx, |s| s.prefs.privacy.lastfm.submit = true));
    cx.update(|cx| {
        let made = track(&song, &tagged, cx).expect("a track");
        assert_eq!((made.artist.as_str(), made.title.as_str()), ("Artist", "Song"));
        assert_eq!(made.album, None, "a blank album says nothing");
        assert_eq!(track(&song, &tags("Artist", "  ", "Album"), cx), None);
    });
    cx.update(|cx| settings::update(cx, |s| s.prefs.privacy.never_remember = vec!["song*".into()]));
    cx.update(|cx| assert_eq!(track(&song, &tagged, cx), None, "excluded from history"));
}

#[gpui::test]
fn without_a_player_nothing_is_followed(cx: &mut TestAppContext) {
    let cx = crate::main_window::tests::open(cx);
    let main = crate::main_window::tests::main_view(cx);
    main.update(cx, |main, cx| {
        main.on_scrobble_event(&PlayerEvent::StateChanged(PlaybackState::Playing), cx);
        assert!(main.scrobbling.track.is_none());
    });
}
