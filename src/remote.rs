//! The web remote (S8.15): a small web page a phone on the same network opens to play, pause,
//! change the volume and pick from the queue, served by the app itself.
//!
//! Safety comes first, since it listens on the network: phones sign in with a password kept in
//! the OS keychain (there is no way in without one), a guessing address is shut out for a
//! minute, only local networks are answered (never the internet, even through a forwarded
//! port), the `Host` must be an address or a `.local` name (no DNS rebinding), and changes must
//! come from the page itself (no cross-site requests). Nothing here touches GPUI: the main
//! window publishes what the page shows and runs the commands it sends
//! (`main_window/web_remote.rs`).

pub mod api;
mod auth;
mod guard;
mod http;
mod server;

use std::sync::Arc;

pub use api::{MAX_QUEUE_ITEMS, Published, QueueItem, QueueList, RemoteCommand, RemoteState};
pub use server::RemoteServer;

/// Where the server reads the password from each time someone signs in, so a password changed
/// in Settings counts at once: the saved one, `None` without one, or why it can't be read.
pub type PasswordSource = Arc<dyn Fn() -> Result<Option<String>, String> + Send + Sync>;

/// The port the web remote listens on by default: next to VLC's usual 8080, which streaming
/// over HTTP takes (S8.12), so both can run at once.
pub const DEFAULT_PORT: u16 = 8081;
