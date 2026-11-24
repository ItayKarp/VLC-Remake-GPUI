//! Player streaming tests (S8.12): a stream belongs to the media it started on, one that
//! couldn't start ends, and casting and streaming don't mix.

use std::net::{Ipv4Addr, SocketAddr};

use gpui::TestAppContext;

use super::Rig;
use crate::app::media::Media;
use crate::engine::backend::{BackendEvent, Command};
use crate::engine::cast::{CastDevice, CastDeviceId};
use crate::engine::mock::MockMedia;
use crate::engine::player::PlayerEvent;
use crate::engine::stream_out::{Contents, Encoding, Local, Protocol, StreamQuality, StreamTarget};
use crate::engine::values::Start;

fn stream() -> StreamTarget {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 8080));
    StreamTarget {
        protocol: Protocol::Http,
        address,
        viewer_host: address.ip(),
        quality: StreamQuality::Medium,
        contents: Contents::Everything { subtitles: true },
        encoding: Encoding::default(),
        local: Local::PlaysHere,
    }
}

fn streaming(rig: &Rig, cx: &mut TestAppContext) -> Option<StreamTarget> {
    rig.player.read_with(cx, |player, _| player.stream_target().copied())
}

fn stream_commands(rig: &Rig) -> Vec<Command> {
    let commands = rig.mock.take_commands().into_iter();
    commands.filter(|command| matches!(command, Command::Stream(_))).collect()
}

#[gpui::test]
fn nothing_streams_without_media(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.run(cx, |player, cx| player.stream(Some(stream()), cx));
    assert_eq!(streaming(&rig, cx), None);
    assert_eq!(stream_commands(&rig), []);
}

#[gpui::test]
fn the_media_open_streams_until_stopped(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, |player, cx| player.stream(Some(stream()), cx));
    assert_eq!(streaming(&rig, cx), Some(stream()));
    rig.run(cx, |player, cx| player.stream(None, cx));
    assert_eq!(streaming(&rig, cx), None);
    assert_eq!(stream_commands(&rig), [Command::Stream(Some(stream())), Command::Stream(None)]);
    assert_eq!(rig.count(&PlayerEvent::StreamChanged), 2);
}

#[gpui::test]
fn other_media_ends_the_stream_unless_it_opens_streaming(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    let start = Start { stream: Some(stream()), ..Start::default() };
    let media = || Media::Url("mock://clip".into());
    rig.run(cx, |player, cx| player.open_at(media(), start, cx));
    assert_eq!(streaming(&rig, cx), Some(stream()));
    rig.open(cx);
    assert_eq!(streaming(&rig, cx), None);
    assert_eq!(rig.count(&PlayerEvent::StreamChanged), 2);
}

#[gpui::test]
fn a_stream_that_couldnt_start_ends(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    rig.run(cx, |player, cx| player.stream(Some(stream()), cx));
    rig.emit(cx, BackendEvent::StreamFailed);
    assert_eq!(streaming(&rig, cx), None);
    assert_eq!(rig.count(&PlayerEvent::StreamFailed), 1);
}

#[gpui::test]
fn casting_and_streaming_stop_each_other(cx: &mut TestAppContext) {
    let rig = Rig::new(cx, MockMedia::default());
    rig.open(cx);
    let tv = CastDevice { id: CastDeviceId(1), name: "TV".into(), video: true };
    rig.emit(cx, BackendEvent::CastDevices(vec![tv]));
    rig.run(cx, |player, cx| player.stream(Some(stream()), cx));
    rig.run(cx, |player, cx| player.cast_to(Some(CastDeviceId(1)), cx));
    assert_eq!(streaming(&rig, cx), None);
    rig.run(cx, |player, cx| player.stream(Some(stream()), cx));
    let cast = rig.player.read_with(cx, |player, _| player.cast_target().cloned());
    assert_eq!(cast, None);
}
