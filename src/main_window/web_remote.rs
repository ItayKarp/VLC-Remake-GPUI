//! The main window's side of the web remote (S8.15): it starts and stops the server as
//! Settings → Controls → Remotes says, publishes what plays and the queue for phones twice a
//! second, and runs the commands they send through the same paths as the window's own buttons.
//!
//! The Web remote control row's hint says where to open it ("Open http://192.168.1.5:8081 on a
//! phone on this network"), or why it isn't running (the port is taken). The OS may ask once
//! whether VLC may accept connections from the network; until it is allowed, phones can't reach
//! the remote.

use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use futures::channel::mpsc::unbounded;
use gpui::{App, Context, Global, Task, Window};

use super::MainWindow;
use crate::engine::player::Player;
use crate::engine::stream_out::network_address;
use crate::engine::values::Volume;
use crate::privacy::keychain::{self, Secret};
use crate::remote::{
    MAX_QUEUE_ITEMS, PasswordSource, Published, QueueItem, QueueList, RemoteCommand, RemoteServer,
    RemoteState,
};
use crate::settings;

/// How often what plays is published for phones.
const PUBLISH_EVERY: Duration = Duration::from_millis(500);

/// The web remote, while Settings asks for it.
#[derive(Default)]
pub(super) struct WebRemote {
    /// The port last asked for, whether or not it could listen there; `None` while it's off.
    wanted: Option<u16>,
    running: Option<Running>,
}

struct Running {
    _server: RemoteServer,
    published: Published,
    /// The queue's revision last published.
    queue_revision: Option<u64>,
    _commands: Task<()>,
    _publishing: Task<()>,
}

/// What the Web remote control row says under its label.
#[derive(Default)]
struct WebRemoteStatus(Option<String>);

impl Global for WebRemoteStatus {}

/// The Web remote control row's hint: where to open the remote, or why it isn't running.
pub fn web_remote_hint(cx: &App) -> Option<String> {
    cx.try_global::<WebRemoteStatus>()?.0.clone()
}

impl MainWindow {
    /// Starts, moves or stops the web remote to match Settings → Controls → Remotes.
    pub(super) fn sync_web_remote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prefs = &settings::get(cx).prefs.controls.web_remote;
        let wanted = prefs.enabled.then_some(prefs.port);
        if wanted == self.web_remote.wanted {
            return;
        }
        self.web_remote.wanted = wanted;
        // The old server goes first, so a restart on the same port finds it free.
        self.web_remote.running = None;
        let status = wanted.map(|port| match self.start_web_remote(port, window, cx) {
            Ok(()) => listening_message(network_address(), port),
            Err(error) => failure_message(&error, port),
        });
        cx.set_global(WebRemoteStatus(status));
        cx.refresh_windows();
    }

    fn start_web_remote(
        &mut self,
        port: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> io::Result<()> {
        let published = Published::default();
        let (sender, mut receiver) = unbounded();
        let password: PasswordSource = Arc::new(|| {
            keychain::load(Secret::WebRemotePassword).map_err(|error| error.message().to_owned())
        });
        let server = RemoteServer::start(port, published.clone(), sender, password)?;
        let commands = cx.spawn_in(window, async move |this, cx| {
            let answer = |this: &mut Self, command, window: &mut Window, cx: &mut Context<Self>| {
                this.answer(command, window, cx);
            };
            while let Some(command) = receiver.next().await
                && this.update_in(cx, |this, window, cx| answer(this, command, window, cx)).is_ok()
            {
            }
        });
        let publishing = cx.spawn(async move |this, cx| {
            let publish = |this: &mut Self, cx: &mut Context<Self>| this.publish_to_remote(cx);
            while this.update(cx, publish).is_ok() {
                cx.background_executor().timer(PUBLISH_EVERY).await;
            }
        });
        self.web_remote.running = Some(Running {
            _server: server,
            published,
            queue_revision: None,
            _commands: commands,
            _publishing: publishing,
        });
        Ok(())
    }

    /// Runs a phone's command and publishes what it changed at once.
    fn answer(&mut self, command: RemoteCommand, window: &mut Window, cx: &mut Context<Self>) {
        self.run_remote_command(command, window, cx);
        self.publish_to_remote(cx);
    }

    /// Does what a phone asked, as the window's own controls would.
    fn run_remote_command(
        &mut self,
        command: RemoteCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            RemoteCommand::PlayPause => self.play_pause(window, cx),
            RemoteCommand::Previous => self.previous(window, cx),
            RemoteCommand::Next => self.next(window, cx),
            RemoteCommand::Volume { percent } => {
                let Some(player) = self.audible(cx) else { return };
                let volume = Volume::from_percent(percent);
                player.update(cx, |player, cx| player.set_volume(volume, cx));
                self.show_volume(window, cx);
            }
            RemoteCommand::ToggleMute => {
                let Some(player) = self.audible(cx) else { return };
                player.update(cx, Player::toggle_mute);
                self.show_volume(window, cx);
            }
            RemoteCommand::Play { id } => {
                let mut entries = self.queue.entries().iter();
                if let Some(index) = entries.position(|entry| entry.id.number() == id) {
                    self.play(index, window, cx);
                }
            }
        }
    }

    /// Hands the server what plays now, and the queue when it changed.
    fn publish_to_remote(&mut self, cx: &mut Context<Self>) {
        let state = self.remote_state(cx);
        let queue = (self.web_remote.running.as_ref())
            .filter(|running| running.queue_revision != Some(state.queue_revision))
            .map(|_| self.remote_queue());
        let Some(running) = &mut self.web_remote.running else { return };
        if let Some(queue) = queue {
            running.queue_revision = Some(state.queue_revision);
            running.published.set_queue(queue);
        }
        running.published.set_state(state);
    }

    fn remote_state(&self, cx: &App) -> RemoteState {
        let loaded = self.loaded_player(cx);
        let loaded = loaded.as_ref().map(|player| player.read(cx));
        let player = self.player.as_ref().map(|(player, _)| player.read(cx));
        let millis = |duration: Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        RemoteState {
            title: self.title(cx),
            playing: loaded.is_some_and(Player::wants_to_play),
            position_ms: loaded.map_or(0, |player| millis(player.position())),
            duration_ms: loaded.and_then(Player::duration).map(millis),
            volume: player.map_or(Volume::FULL, Player::volume).as_percent(),
            max_volume: player.map_or(Volume::FULL, Player::max_volume).as_percent(),
            muted: player.is_some_and(Player::is_muted),
            can_go_back: self.can_go_back(cx),
            can_go_next: self.can_go_next(),
            queue_revision: self.queue.revision(),
        }
    }

    fn remote_queue(&self) -> QueueList {
        let current = self.queue.current_index();
        let entries = self.queue.entries();
        let items = entries.iter().take(MAX_QUEUE_ITEMS).enumerate();
        QueueList {
            items: items
                .map(|(index, entry)| QueueItem {
                    id: entry.id.number(),
                    title: entry.media.display_name(),
                    current: current == Some(index),
                })
                .collect(),
            more: entries.len().saturating_sub(MAX_QUEUE_ITEMS),
        }
    }
}

/// The hint while the remote listens on `port`, at this computer's `address` on its network.
fn listening_message(address: Option<IpAddr>, port: u16) -> String {
    match address {
        Some(address) => format!("Open http://{address}:{port} on a phone on this network."),
        None => format!(
            "This computer isn't on a network, so only it can open the remote, at \
             http://localhost:{port}."
        ),
    }
}

/// The hint when the remote couldn't listen on `port`.
fn failure_message(error: &io::Error, port: u16) -> String {
    tracing::warn!("web remote: can't listen on port {port}: {error}");
    match error.kind() {
        io::ErrorKind::AddrInUse => {
            format!("Port {port} is in use by another program. Pick another port.")
        }
        io::ErrorKind::PermissionDenied => {
            format!("VLC isn't allowed to listen on port {port}. Pick another port.")
        }
        _ => format!("The web remote couldn't start on port {port}: {error}."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
