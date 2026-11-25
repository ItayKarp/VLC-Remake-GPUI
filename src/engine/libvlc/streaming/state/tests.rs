use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use super::*;
use crate::engine::stream_out::{Contents, Encoding, Protocol, StreamQuality};

/// `libvlc_state_t`'s NothingSpecial, Playing and Ended.
const NOTHING_SPECIAL: c_int = 0;
const PLAYING: c_int = 3;
const ENDED: c_int = 6;

fn target(local: Local) -> StreamTarget {
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080));
    StreamTarget {
        protocol: Protocol::Http,
        address,
        viewer_host: address.ip(),
        quality: StreamQuality::Medium,
        contents: Contents::Everything { subtitles: true },
        encoding: Encoding::default(),
        local,
    }
}

fn secs(secs: u64) -> Duration {
    Duration::from_secs(secs)
}

/// Streaming that started on media at `from`.
fn streaming_from(from: Duration) -> Streaming {
    let mut streaming = Streaming::default();
    streaming.open(Some(target(Local::PlaysHere)), from);
    streaming
}

#[test]
fn the_media_state_reads_the_player_only_while_media_is_open() {
    assert_eq!(MediaState::of(false, PLAYING), MediaState::Closed);
    assert_eq!(MediaState::of(true, PLAYING), MediaState::Going);
    assert_eq!(MediaState::of(true, 1), MediaState::Going, "opening is going");
    assert_eq!(MediaState::of(true, 4), MediaState::Going, "paused is going");
    assert_eq!(MediaState::of(true, ENDED), MediaState::Stopped);
    assert_eq!(MediaState::of(true, NOTHING_SPECIAL), MediaState::Stopped);
}

#[test]
fn starting_a_stream_reopens_going_media_and_prepares_stopped_media() {
    let stream = Some(target(Local::PlaysHere));
    let mut going = Streaming::default();
    assert_eq!(going.retarget(stream, secs(5), MediaState::Going), Retarget::Reopen);
    let mut stopped = Streaming::default();
    assert_eq!(stopped.retarget(stream, secs(5), MediaState::Stopped), Retarget::Prepare);
    assert!(going.is_streaming() && stopped.is_streaming());
}

#[test]
fn without_media_a_stream_is_kept_for_the_next_open_and_asks_nothing() {
    let mut streaming = Streaming::default();
    let retarget = streaming.retarget(Some(target(Local::PlaysHere)), secs(0), MediaState::Closed);
    assert_eq!(retarget, Retarget::Nothing);
    assert!(streaming.is_streaming());
}

#[test]
fn asking_for_the_stream_already_going_changes_nothing() {
    let mut streaming = streaming_from(secs(3));
    let same = Some(target(Local::PlaysHere));
    assert_eq!(streaming.retarget(same, secs(40), MediaState::Going), Retarget::Nothing);
    assert_eq!(streaming.fail(false, MediaState::Going), Some(secs(3)), "it still started at 3 s");
    let mut idle = Streaming::default();
    assert_eq!(idle.retarget(None, secs(1), MediaState::Going), Retarget::Nothing);
}

#[test]
fn stopping_a_stream_reopens_the_media_without_it() {
    let mut streaming = streaming_from(secs(3));
    assert_eq!(streaming.retarget(None, secs(9), MediaState::Going), Retarget::Reopen);
    assert!(!streaming.is_streaming());
    assert_eq!(streaming.media_options(), None);
}

#[test]
fn new_media_gets_the_streams_options_while_streaming() {
    assert_eq!(Streaming::default().media_options(), None);
    let stream = target(Local::PlaysHere);
    assert_eq!(streaming_from(secs(0)).media_options(), Some(stream.media_options()));
}

#[test]
fn only_a_stream_only_target_keeps_the_media_from_playing_here() {
    assert!(!Streaming::default().streams_only());
    assert!(!streaming_from(secs(0)).streams_only());
    let mut stream_only = Streaming::default();
    stream_only.open(Some(target(Local::StreamOnly)), secs(0));
    assert!(stream_only.streams_only());
}

#[test]
fn a_stream_that_never_started_goes_on_without_it_from_where_it_started() {
    let mut streaming = Streaming::default();
    streaming.retarget(Some(target(Local::PlaysHere)), secs(12), MediaState::Going);
    assert_eq!(streaming.fail(false, MediaState::Going), Some(secs(12)));
    assert!(!streaming.is_streaming(), "the failed stream is dropped");
    assert_eq!(streaming.fail(false, MediaState::Going), None, "and fails only once");
}

#[test]
fn other_failures_are_the_medias_own() {
    assert_eq!(Streaming::default().fail(false, MediaState::Going), None, "not streaming");
    let mut started = streaming_from(secs(2));
    assert_eq!(started.fail(true, MediaState::Going), None, "the stream had started");
    assert!(started.is_streaming());
    let mut closed = streaming_from(secs(2));
    assert_eq!(closed.fail(false, MediaState::Closed), None, "no media to reopen");
}
