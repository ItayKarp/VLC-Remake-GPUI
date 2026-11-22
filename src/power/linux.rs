//! Linux: `org.freedesktop.ScreenSaver.Inhibit` on the session bus (GNOME, KDE, Xfce and most
//! others), or, without it, a `systemd-inhibit` child holding an idle lock.
//!
//! The inhibition lasts as long as the D-Bus connection or the child, so a thread holds it.
//! Nothing here blocks the UI: the request is made on that thread, and dropping only signals it.

use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{Context as _, Result};
use zbus::blocking::Connection;

const SERVICE: &str = "org.freedesktop.ScreenSaver";
const PATH: &str = "/org/freedesktop/ScreenSaver";

/// A held inhibition, released (by its thread) on drop.
pub struct Request {
    /// Dropping it wakes the thread, which releases the inhibition and ends.
    _release: Sender<()>,
}

impl Request {
    /// Starts the thread that inhibits the screen saver.
    pub fn new(app: &str, reason: &str) -> Result<Self> {
        let (release, released) = mpsc::channel();
        let (app, reason) = (app.to_owned(), reason.to_owned());
        thread::Builder::new()
            .name("keep-awake".into())
            .spawn(move || hold(&app, &reason, &released))
            .context("starting the keep-awake thread")?;
        Ok(Self { _release: release })
    }
}

/// Inhibits until `released` says to stop (or its sender is dropped), then releases.
fn hold(app: &str, reason: &str, released: &Receiver<()>) {
    match inhibit_screen_saver(app, reason) {
        Ok((connection, cookie)) => hold_screen_saver(&connection, cookie, released),
        Err(error) => {
            tracing::info!("power: no screen saver service ({error:#}); trying systemd-inhibit");
            hold_idle_lock(app, reason, released);
        }
    }
}

/// Waits for `released`, then hands the screen saver its `cookie` back.
fn hold_screen_saver(connection: &Connection, cookie: u32, released: &Receiver<()>) {
    let _ = released.recv();
    let uninhibited =
        connection.call_method(Some(SERVICE), PATH, Some(SERVICE), "UnInhibit", &cookie);
    if let Err(error) = uninhibited {
        tracing::warn!("power: UnInhibit failed: {error}");
    }
}

/// Holds a `systemd-inhibit` idle lock until `released`, then ends it.
fn hold_idle_lock(app: &str, reason: &str, released: &Receiver<()>) {
    let mut child = match systemd_inhibit(app, reason) {
        Ok(child) => child,
        Err(error) => {
            tracing::warn!("power: couldn't keep the display awake: {error:#}");
            return;
        }
    };
    let _ = released.recv();
    let _ = child.kill();
    let _ = child.wait();
}

fn inhibit_screen_saver(app: &str, reason: &str) -> Result<(Connection, u32)> {
    let connection = Connection::session().context("connecting to the session bus")?;
    let reply = connection
        .call_method(Some(SERVICE), PATH, Some(SERVICE), "Inhibit", &(app, reason))
        .context("ScreenSaver.Inhibit")?;
    let cookie: u32 = reply.body().deserialize().context("reading the inhibit cookie")?;
    Ok((connection, cookie))
}

fn systemd_inhibit(app: &str, reason: &str) -> Result<Child> {
    Command::new("systemd-inhibit")
        .args(["--what=idle", "--mode=block"])
        .arg(format!("--who={app}"))
        .arg(format!("--why={reason}"))
        .args(["sleep", "infinity"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("starting systemd-inhibit")
}
