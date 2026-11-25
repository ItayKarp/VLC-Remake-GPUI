//! Window tests of the Stream dialog: what Stream starts from the fields and Settings →
//! Recording, what it refuses and why, and Stop and Copy Link while a stream runs.

use std::net::{Ipv4Addr, SocketAddr, TcpListener};

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::*;
use crate::main_window::device_dialog::tests::{Handed, draw, open_view, recorder};

/// Opens the dialog streaming a clip, which streams to `running` now if it does.
fn open(
    cx: &mut TestAppContext,
    running: Option<StreamTarget>,
) -> (Entity<StreamDialog>, Handed<Option<StreamTarget>>, &mut VisualTestContext) {
    let (handed, on_choose) = recorder();
    let media = Media::Url("https://example.com/clip.mp4".into());
    let (dialog, cx) =
        open_view(cx, move |window, cx| StreamDialog::new(media, running, on_choose, window, cx));
    (dialog, handed, cx)
}

/// A port nothing listens on now.
fn free_port() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    listener.local_addr().expect("a bound address").port()
}

fn set_text(
    field: impl Fn(&StreamDialog) -> &Entity<TextInput>,
    text: &str,
    dialog: &Entity<StreamDialog>,
    cx: &mut VisualTestContext,
) {
    let input = dialog.read_with(cx, |dialog, _| field(dialog).clone());
    input.update(cx, |input, cx| input.set_text(text, cx));
    draw(cx);
}

fn set_port(text: &str, dialog: &Entity<StreamDialog>, cx: &mut VisualTestContext) {
    set_text(|dialog| &dialog.port, text, dialog, cx);
}

fn port(dialog: &Entity<StreamDialog>, cx: &mut VisualTestContext) -> String {
    dialog.read_with(cx, |dialog, cx| dialog.port.read(cx).text().to_owned())
}

/// Presses Stream (or Stop) and draws what follows.
fn confirm(dialog: &Entity<StreamDialog>, cx: &mut VisualTestContext) {
    dialog.update_in(cx, StreamDialog::confirm);
    draw(cx);
}

fn problem(dialog: &Entity<StreamDialog>, cx: &mut VisualTestContext) -> Option<String> {
    dialog.read_with(cx, |dialog, _| dialog.problem.as_ref().map(ToString::to_string))
}

/// A stream running as `protocol` to `address`.
fn running(protocol: Protocol, address: SocketAddr) -> StreamTarget {
    StreamTarget {
        protocol,
        address,
        viewer_host: address.ip(),
        quality: StreamQuality::default(),
        contents: Contents::Everything { subtitles: true },
        encoding: Encoding::default(),
        local: Local::PlaysHere,
    }
}

#[test]
fn ports_are_numbers_in_range() {
    assert_eq!(parse_port(" 8080 "), Ok(8080));
    assert!(parse_port("0").is_err());
    assert!(parse_port("70000").is_err());
    assert!(parse_port("http").is_err());
}

#[test]
fn streams_play_here_as_settings_say() {
    let mut recording = RecordingSettings::default();
    assert_eq!(local(&recording), Local::PlaysHere);
    recording.play_locally_while_streaming = false;
    assert_eq!(local(&recording), Local::StreamOnly);
}

#[gpui::test]
fn stream_starts_on_this_computer_at_the_port_and_quality_picked(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, None);
    assert_eq!(port(&dialog, cx), "8080", "HTTP's usual port to start with");
    let port = free_port();
    set_port(&port.to_string(), &dialog, cx);
    let qualities = dialog.read_with(cx, |dialog, _| dialog.qualities.clone());
    let low = StreamQuality::ALL.iter().position(|&q| q == StreamQuality::Low).expect("Low");
    qualities.update(cx, |list, cx| list.set_selected(low, cx));
    confirm(&dialog, cx);
    let handed = handed.borrow();
    let [Some(target)] = handed.as_slice() else { panic!("one stream started: {handed:?}") };
    assert_eq!(target.protocol, Protocol::Http);
    assert_eq!(target.address, SocketAddr::from((Ipv4Addr::LOCALHOST, port)));
    assert!(!target.is_open_to_network());
    assert_eq!(target.quality, StreamQuality::Low);
    assert_eq!(target.contents, Contents::Everything { subtitles: true });
    assert_eq!(target.local, Local::PlaysHere);
}

#[gpui::test]
fn a_port_in_use_is_refused_until_it_is_edited(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, None);
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let port = taken.local_addr().expect("a bound address").port();
    set_port(&port.to_string(), &dialog, cx);
    confirm(&dialog, cx);
    let why = problem(&dialog, cx).expect("Stream was refused");
    assert_eq!(why, format!("Port {port} is in use by another program. Try another one."));
    assert!(handed.borrow().is_empty(), "nothing started");
    let input = dialog.read_with(cx, |dialog, _| dialog.port.clone());
    cx.update(|window, cx| input.read(cx).focus_handle(cx).focus(window, cx));
    input.update(cx, TextInput::select_everything);
    draw(cx);
    cx.simulate_input("9");
    draw(cx);
    assert_eq!(problem(&dialog, cx), None, "editing the port clears why");
}

#[gpui::test]
fn a_port_must_be_a_number(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, None);
    set_port("http", &dialog, cx);
    confirm(&dialog, cx);
    let why = problem(&dialog, cx).expect("Stream was refused");
    assert!(why.contains("isn't a port"), "{why}");
    assert!(handed.borrow().is_empty());
}

#[gpui::test]
fn udp_sends_to_the_address_typed(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, None);
    dialog.update(cx, |dialog, cx| dialog.set_protocol(Protocol::Udp, cx));
    draw(cx);
    assert_eq!(port(&dialog, cx), "1234", "the usual port follows the protocol");
    set_text(|dialog| &dialog.send_to, "the tv", &dialog, cx);
    confirm(&dialog, cx);
    let why = problem(&dialog, cx).expect("Stream was refused");
    assert!(why.contains("isn't an IP address"), "{why}");
    set_text(|dialog| &dialog.send_to, "192.168.1.20", &dialog, cx);
    confirm(&dialog, cx);
    let handed = handed.borrow();
    let [Some(target)] = handed.as_slice() else { panic!("one stream started: {handed:?}") };
    assert_eq!(target.address, "192.168.1.20:1234".parse().expect("an address"));
    assert_eq!(target.url(), "udp://@:1234");
}

#[gpui::test]
fn a_port_typed_stays_when_the_protocol_changes(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx, None);
    dialog.update(cx, |dialog, cx| dialog.set_protocol(Protocol::Rtsp, cx));
    draw(cx);
    assert_eq!(port(&dialog, cx), "8554");
    set_port("9000", &dialog, cx);
    dialog.update(cx, |dialog, cx| dialog.set_protocol(Protocol::Http, cx));
    draw(cx);
    assert_eq!(port(&dialog, cx), "9000");
}

#[gpui::test]
fn the_network_reach_listens_on_every_network(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx, None);
    dialog.update(cx, |dialog, cx| dialog.set_reach(Reach::Network, cx));
    draw(cx);
    let (address, viewer_host) =
        dialog.read_with(cx, |dialog, cx| dialog.addresses(8080, cx)).expect("addresses");
    assert_eq!(address, SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080)));
    assert!(!viewer_host.is_unspecified(), "viewers get an address they can open");
}

#[gpui::test]
fn nothing_streams_when_settings_send_neither_sound_nor_picture(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, None);
    cx.update(|_, cx| {
        settings::update(cx, |settings| {
            settings.prefs.recording.stream_audio = false;
            settings.prefs.recording.stream_video = false;
        });
    });
    set_port(&free_port().to_string(), &dialog, cx);
    confirm(&dialog, cx);
    let why = problem(&dialog, cx).expect("Stream was refused");
    assert!(why.contains("neither audio nor video"), "{why}");
    assert!(handed.borrow().is_empty());
}

#[gpui::test]
fn a_running_stream_offers_its_link_and_stop(cx: &mut TestAppContext) {
    let target = running(Protocol::Http, SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080)));
    let (dialog, handed, cx) = open(cx, Some(target));
    dialog.update(cx, |dialog, cx| dialog.copy_link(cx));
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied, Some(target.url()));
    confirm(&dialog, cx);
    assert_eq!(*handed.borrow(), [None], "Stop ends the stream");
}

#[gpui::test]
fn a_udp_stream_says_where_it_goes(cx: &mut TestAppContext) {
    let target = running(Protocol::Udp, "192.168.1.20:5000".parse().expect("an address"));
    let (dialog, _, cx) = open(cx, Some(target));
    dialog.update(cx, |dialog, cx| dialog.copy_link(cx));
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("udp://@:5000"));
}
