//! Single instance (S0.6.1): the first launch becomes the running instance; later
//! launches forward what they were asked to open to it and exit.
//!
//! Everything here runs before GPUI starts. Forwarded launches (and macOS "open
//! these files" events) arrive in the [`Inbox`], which the UI drains
//! (`app::open::receive`).

pub mod batch;
mod client;
mod lock;
mod server;
mod wire;

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use interprocess::local_socket::Stream;
use interprocess::local_socket::prelude::*;

use super::request::{Launch, OpenRequest, StartScreen};
use crate::app::media::Media;
use crate::settings;
pub use client::FORWARD_TIMEOUT;
use lock::InstancePaths;
pub use server::Server;

/// The byte the running instance answers with once it has taken a launch.
const ACK: u8 = 0x06;
/// How long either side waits for the other to read or write.
const IO_TIMEOUT: Duration = Duration::from_secs(2);

/// Makes reads on `stream` give up after [`IO_TIMEOUT`] where the OS supports it.
/// Windows named pipes don't: there the sender's own deadline
/// ([`FORWARD_TIMEOUT`]) still holds, and a stalled sender only ties up the
/// listener's thread for that one connection.
fn limit_reads(stream: &Stream) -> io::Result<()> {
    match stream.set_recv_timeout(Some(IO_TIMEOUT)) {
        Err(err) if err.kind() == io::ErrorKind::Unsupported => Ok(()),
        result => result,
    }
}

/// One launch waiting for the UI, with the sender waiting for its acknowledgement.
pub struct Incoming {
    /// What was asked for.
    pub launch: Launch,
    ack: Option<mpsc::Sender<()>>,
}

impl Incoming {
    /// Tells the sender the UI has the launch, so it can exit.
    pub fn acknowledge(&mut self) {
        if let Some(ack) = self.ack.take() {
            ack.send(()).ok();
        }
    }
}

/// Where launches for the UI are posted, from any thread.
#[derive(Clone)]
pub struct Inbox {
    sender: UnboundedSender<Incoming>,
    ui_running: Arc<AtomicBool>,
}

/// The UI's end of the [`Inbox`].
pub struct InboxReceiver {
    /// Launches in arrival order.
    pub receiver: UnboundedReceiver<Incoming>,
    ui_running: Arc<AtomicBool>,
}

impl InboxReceiver {
    /// Marks the UI as draining the inbox: from now on, a sender is acknowledged
    /// only once the UI thread has its launch.
    pub fn mark_running(&self) {
        self.ui_running.store(true, Ordering::Release);
    }
}

/// Creates the inbox and its receiving end.
pub fn inbox() -> (Inbox, InboxReceiver) {
    let (sender, receiver) = unbounded();
    let ui_running = Arc::new(AtomicBool::new(false));
    (Inbox { sender, ui_running: ui_running.clone() }, InboxReceiver { receiver, ui_running })
}

impl Inbox {
    /// Posts URLs the OS asked us to open (macOS Finder "Open With", files dropped on
    /// the Dock icon), as a launch that replaces the queue.
    pub fn post_urls(&self, urls: &[String]) {
        let items: Vec<Media> = urls.iter().map(|url| Media::from_url(url)).collect();
        let launch = Launch { open: OpenRequest::replace(items), start: StartScreen::Auto };
        self.sender.unbounded_send(Incoming { launch, ack: None }).ok();
    }
}

/// What this process is, after looking for a running instance.
pub enum Role {
    /// The running instance: it holds the lock and listens for later launches.
    Primary(Server),
    /// Runs on its own: multiple instances are allowed, the gallery was asked for,
    /// or the lock couldn't be used.
    Standalone,
    /// Another instance took this launch; exit now.
    Forwarded,
    /// Another instance holds the lock but didn't take the launch in time.
    NotResponding,
}

/// Becomes the running instance, or forwards `launch` to the one that is.
pub fn acquire(launch: &Launch, inbox: &Inbox) -> Role {
    if launch.start == StartScreen::Gallery || settings::peek().library.allow_multiple_instances {
        return Role::Standalone;
    }
    let Some(paths) = InstancePaths::resolve() else {
        return Role::Standalone;
    };
    acquire_at(&paths, launch, inbox)
}

// Each outcome logs in its own function: with `tracing`'s `log` feature (on in Linux
// builds) every macro adds enough branches to push one function past the complexity limit.
fn acquire_at(paths: &InstancePaths, launch: &Launch, inbox: &Inbox) -> Role {
    match paths.try_lock() {
        Ok(Some(lock)) => serve(paths, lock, inbox),
        Ok(None) => forward(paths, launch),
        Err(err) => {
            tracing::warn!("instance: couldn't use {}: {err}", paths.lock_file.display());
            Role::Standalone
        }
    }
}

/// Becomes the instance, listening for later launches.
fn serve(paths: &InstancePaths, lock: std::fs::File, inbox: &Inbox) -> Role {
    server::listen(paths, lock, inbox).map_or_else(
        |err| {
            tracing::warn!("instance: couldn't listen for other launches: {err}");
            Role::Standalone
        },
        Role::Primary,
    )
}

/// Hands this launch to the running instance.
fn forward(paths: &InstancePaths, launch: &Launch) -> Role {
    client::forward(paths, launch).map_or_else(
        |err| {
            tracing::warn!("instance: the running instance didn't take the launch: {err}");
            Role::NotResponding
        },
        |()| Role::Forwarded,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use futures::StreamExt;

    use super::*;

    #[test]
    fn a_second_launch_is_forwarded_to_the_first() {
        let dir = std::env::temp_dir().join(format!("rvlc-instance-{}", std::process::id()));
        let paths = InstancePaths::in_dir(&dir);
        let (inbox, mut receiver) = inbox();
        let Role::Primary(server) = acquire_at(&paths, &Launch::default(), &inbox) else {
            panic!("the first launch should become the instance");
        };

        let file = Media::File(PathBuf::from("/videos/a.mkv"));
        let second = Launch { open: OpenRequest::replace(vec![file]), ..Launch::default() };
        let (other_inbox, _) = super::inbox();
        assert!(matches!(acquire_at(&paths, &second, &other_inbox), Role::Forwarded));

        let forwarded = futures::executor::block_on(receiver.receiver.next());
        assert_eq!(forwarded.map(|incoming| incoming.launch), Some(second));
        drop(server);
        std::fs::remove_dir_all(dir).ok();
    }
}
