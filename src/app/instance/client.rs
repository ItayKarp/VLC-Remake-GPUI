//! A later launch's side: hand the launch to the running instance, or give up after
//! [`FORWARD_TIMEOUT`] so a hung instance doesn't hang this process too.

use std::io::{self, Read};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use interprocess::local_socket::Stream;
use interprocess::local_socket::prelude::*;

use super::lock::InstancePaths;
use super::{ACK, limit_reads, wire};
use crate::app::request::Launch;

/// How long the whole hand-off may take before the running instance counts as not
/// responding.
pub const FORWARD_TIMEOUT: Duration = Duration::from_secs(2);
/// How often to retry connecting while the running instance is still starting up.
const CONNECT_RETRY: Duration = Duration::from_millis(50);

/// Sends `launch` to the running instance. Fails if it doesn't accept it within
/// [`FORWARD_TIMEOUT`].
pub fn forward(paths: &InstancePaths, launch: &Launch) -> io::Result<()> {
    let deadline = Instant::now() + FORWARD_TIMEOUT;
    let (done, result) = mpsc::channel();
    let paths = paths.clone();
    let launch = launch.clone();
    // On a thread, because a hung peer can block a pipe read or write past any
    // timeout the OS offers.
    thread::Builder::new().name("instance-forward".into()).spawn(move || {
        done.send(send(&paths, &launch, deadline)).ok();
    })?;
    result
        .recv_timeout(FORWARD_TIMEOUT)
        .unwrap_or_else(|_| Err(io::Error::new(io::ErrorKind::TimedOut, "no answer")))
}

fn send(paths: &InstancePaths, launch: &Launch, deadline: Instant) -> io::Result<()> {
    let mut stream = connect(paths, deadline)?;
    limit_reads(&stream)?;
    let owner = wire::read_u32(&mut stream)?;
    allow_foreground(owner);
    wire::write_frame(&mut stream, launch)?;
    let mut ack = [0];
    stream.read_exact(&mut ack)?;
    if ack == [ACK] { Ok(()) } else { Err(io::Error::other("unexpected answer")) }
}

/// Connects, retrying while the instance holds the lock but isn't listening yet
/// (it's starting up).
fn connect(paths: &InstancePaths, deadline: Instant) -> io::Result<Stream> {
    loop {
        match Stream::connect(paths.name()?) {
            Ok(stream) => return Ok(stream),
            Err(err) if Instant::now() + CONNECT_RETRY < deadline => {
                tracing::debug!("instance: connect failed ({err}); retrying");
                thread::sleep(CONNECT_RETRY);
            }
            Err(err) => return Err(err),
        }
    }
}

/// Lets the running instance bring its window to the front. Windows only lets the
/// foreground process do that, and that's us: the user just launched us.
#[cfg(windows)]
fn allow_foreground(process_id: u32) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
    }
    // SAFETY: takes a plain integer; an unknown process ID only makes it fail.
    if unsafe { AllowSetForegroundWindow(process_id) } == 0 {
        tracing::debug!("instance: AllowSetForegroundWindow({process_id}) failed");
    }
}

/// Other platforms let the running instance activate itself.
#[cfg(not(windows))]
fn allow_foreground(_process_id: u32) {}
