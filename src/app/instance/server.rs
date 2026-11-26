//! The running instance's side: accept launches forwarded by later processes.
//!
//! Each connection gets this process's ID (so the sender can let us take the
//! foreground on Windows), then sends one frame, then waits for one acknowledgement
//! byte. Once the UI is running, the acknowledgement comes from the UI thread, so a
//! hung UI shows up as a timeout in the sender instead of files vanishing silently.

use std::fs::File;
use std::io::{self, Write};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::thread;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{ListenerOptions, Stream};

use super::lock::InstancePaths;
use super::{ACK, IO_TIMEOUT, Inbox, Incoming, limit_reads, wire};

/// Holds the instance lock for as long as the app runs. The listener thread keeps
/// accepting on its own.
pub struct Server {
    _lock: File,
}

/// Starts listening for forwarded launches, delivering them to `inbox`.
pub fn listen(paths: &InstancePaths, lock: File, inbox: &Inbox) -> io::Result<Server> {
    paths.remove_stale_socket();
    let listener = ListenerOptions::new().name(paths.name()?).reclaim_name(true).create_sync()?;
    paths.restrict_socket()?;
    let inbox = inbox.clone();
    thread::Builder::new().name("instance-listener".into()).spawn(move || {
        for connection in listener.incoming() {
            match connection {
                Ok(stream) => serve_in_thread(stream, inbox.clone()),
                Err(err) => tracing::warn!("instance: accepting a connection failed: {err}"),
            }
        }
    })?;
    Ok(Server { _lock: lock })
}

/// Serves one connection on its own thread, so a slow sender can't hold up others.
fn serve_in_thread(stream: Stream, inbox: Inbox) {
    let spawned = thread::Builder::new().name("instance-connection".into()).spawn(move || {
        if let Err(err) = serve(stream, &inbox) {
            tracing::warn!("instance: dropped a forwarded launch: {err}");
        }
    });
    if let Err(err) = spawned {
        tracing::warn!("instance: couldn't serve a connection: {err}");
    }
}

fn serve(mut stream: Stream, inbox: &Inbox) -> io::Result<()> {
    limit_reads(&stream)?;
    stream.write_all(&std::process::id().to_le_bytes())?;
    let launch = wire::read_frame(&mut stream)?;
    tracing::debug!("instance: received a launch: {}", launch.summary());

    // Before the UI loop runs, launches queue up and are acknowledged right away:
    // the app is starting, not hung.
    let ui_running = inbox.ui_running.load(Ordering::Acquire);
    let (ack, acked) = mpsc::channel();
    inbox
        .sender
        .unbounded_send(Incoming { launch, ack: ui_running.then_some(ack) })
        .map_err(|_| io::Error::other("the app is shutting down"))?;
    if ui_running {
        acked.recv_timeout(IO_TIMEOUT).map_err(|_| io::Error::other("the UI didn't respond"))?;
    }
    stream.write_all(&[ACK])
}
