//! Tests of the main window's side of the web remote: it follows Settings → Controls →
//! Remotes, the hint says where to open it or why it isn't running, what plays and the queue
//! are published, and a phone's commands work as the window's own buttons do.

use std::net::{Ipv4Addr, TcpListener};

use gpui::{TestAppContext, VisualTestContext};

use super::*;
use crate::app::media::Media;
use crate::main_window::device_dialog::tests::{draw, play_mock};
use crate::main_window::tests::{main_view, open};

/// A port nothing listens on now, on any network.
fn free_port() -> u16 {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("a free port");
    listener.local_addr().expect("a bound address").port()
}

/// How many free ports a test tries before giving up.
const PORT_TRIES: usize = 5;

/// Turns the web remote on at a free port and answers it. A port found free may be taken by
/// another test before the remote binds it, so a few are tried.
fn start_remote(cx: &mut VisualTestContext) -> u16 {
    for _ in 0..PORT_TRIES {
        let port = free_port();
        set_remote(Some(port), cx);
        if is_running(cx) {
            return port;
        }
        set_remote(None, cx);
    }
    panic!("the web remote didn't start on any of {PORT_TRIES} free ports");
}

/// Turns the web remote on at `port`, or off with `None`, as Settings does.
fn set_remote(port: Option<u16>, cx: &mut VisualTestContext) {
    cx.update(|_, cx| {
        settings::update(cx, |settings| {
            let remote = &mut settings.prefs.controls.web_remote;
            remote.enabled = port.is_some();
            remote.port = port.unwrap_or(remote.port);
        });
    });
    draw(cx);
}

fn hint(cx: &mut VisualTestContext) -> Option<String> {
    cx.update(|_, cx| web_remote_hint(cx))
}

fn is_running(cx: &mut VisualTestContext) -> bool {
    let main = main_view(cx);
    main.read_with(cx, |main, _| main.web_remote.running.is_some())
}

/// What the remote publishes now: the state's and the queue's JSON.
fn published(cx: &mut VisualTestContext) -> (String, String) {
    let main = main_view(cx);
    main.update(cx, MainWindow::publish_to_remote);
    main.read_with(cx, |main, _| {
        let running = main.web_remote.running.as_ref().expect("the remote runs");
        (running.published.state_json(), running.published.queue_json())
    })
}

/// Runs a phone's `command` and draws what follows.
fn answer(command: RemoteCommand, cx: &mut VisualTestContext) {
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| main.answer(command, window, cx));
    draw(cx);
}

fn playing_title(cx: &mut VisualTestContext) -> Option<String> {
    let main = main_view(cx);
    main.read_with(cx, |main, cx| {
        let (player, _) = main.player.as_ref()?;
        player.read(cx).media().map(Media::display_name)
    })
}

#[test]
fn the_hint_gives_the_address_to_open() {
    let address = Some("192.168.1.5".parse().unwrap());
    assert_eq!(
        listening_message(address, 8081),
        "Open http://192.168.1.5:8081 on a phone on this network."
    );
    assert!(listening_message(None, 8081).contains("http://localhost:8081"));
}

#[test]
fn the_hint_says_why_it_isnt_running() {
    let taken = io::Error::from(io::ErrorKind::AddrInUse);
    assert_eq!(
        failure_message(&taken, 8081),
        "Port 8081 is in use by another program. Pick another port."
    );
    let refused = io::Error::from(io::ErrorKind::PermissionDenied);
    assert!(failure_message(&refused, 80).contains("isn't allowed"));
    let other = io::Error::other("no sockets left");
    assert_eq!(
        failure_message(&other, 80),
        "The web remote couldn't start on port 80: no sockets left."
    );
}

#[gpui::test]
fn the_remote_runs_while_settings_ask_for_it(cx: &mut TestAppContext) {
    let cx = open(cx);
    assert_eq!(hint(cx), None);
    let port = start_remote(cx);
    assert_eq!(hint(cx), Some(listening_message(network_address(), port)));
    set_remote(None, cx);
    assert!(!is_running(cx));
    assert_eq!(hint(cx), None);
}

#[gpui::test]
fn a_taken_port_says_why_the_remote_isnt_running(cx: &mut TestAppContext) {
    let cx = open(cx);
    let taken = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).expect("a free port");
    let port = taken.local_addr().expect("a bound address").port();
    set_remote(Some(port), cx);
    assert!(!is_running(cx));
    let hint = hint(cx).expect("a hint");
    assert_eq!(hint, format!("Port {port} is in use by another program. Pick another port."));
}

#[gpui::test]
fn what_plays_and_the_queue_are_published(cx: &mut TestAppContext) {
    let cx = open(cx);
    start_remote(cx);
    let (state, queue) = published(cx);
    assert!(state.contains("\"playing\":false"), "{state}");
    assert!(queue.contains("\"items\":[]"), "{queue}");
    play_mock(cx);
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.queue.insert_next(vec![Media::Url("https://e.com/first".into())]);
        main.play(0, window, cx);
    });
    draw(cx);
    let (state, queue) = published(cx);
    assert!(state.contains("\"playing\":true"), "{state}");
    assert!(queue.contains("https://e.com/first") && queue.contains("\"current\":true"), "{queue}");
}

#[gpui::test]
fn a_phones_commands_work_as_the_windows_buttons(cx: &mut TestAppContext) {
    let cx = open(cx);
    let (player, _) = play_mock(cx);
    let main = main_view(cx);
    let ids = main.update_in(cx, |main, window, cx| {
        let both = vec![
            Media::Url("https://e.com/first".into()),
            Media::Url("https://e.com/second".into()),
        ];
        main.queue.replace(both);
        main.play(0, window, cx);
        main.queue.entries().iter().map(|entry| entry.id.number()).collect::<Vec<_>>()
    });
    draw(cx);
    answer(RemoteCommand::Next, cx);
    assert_eq!(playing_title(cx).as_deref(), Some("https://e.com/second"));
    answer(RemoteCommand::Play { id: ids[0] }, cx);
    assert_eq!(playing_title(cx).as_deref(), Some("https://e.com/first"));
    answer(RemoteCommand::Volume { percent: 50 }, cx);
    assert_eq!(player.read_with(cx, |player, _| player.volume().as_percent()), 50);
    answer(RemoteCommand::ToggleMute, cx);
    assert!(player.read_with(cx, |player, _| player.is_muted()));
    answer(RemoteCommand::PlayPause, cx);
    assert!(!player.read_with(cx, |player, _| player.wants_to_play()), "paused");
    answer(RemoteCommand::Previous, cx);
}
