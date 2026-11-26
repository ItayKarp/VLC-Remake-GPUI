//! The web remote's server: a thread that accepts connections on the port, one more thread per
//! connection (a handful at most), and the routes the page calls.
//!
//! Routes: `GET /` is the page; `POST /api/login` signs in with the password and sets the
//! session cookie; `GET /api/state` and `GET /api/queue` give what the app published, and
//! `POST /api/command` sends the app a [`RemoteCommand`], all three for signed-in sessions
//! only. Before any route, [`guard`] refuses other networks, DNS names and other sites.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use futures::channel::mpsc::UnboundedSender;
use serde::Deserialize;

use super::PasswordSource;
use super::api::{Published, RemoteCommand};
use super::auth::{Lockout, Sessions, Token, same_text};
use super::guard;
use super::http::{self, Method, Request, Response, Status};

/// The page phones open.
const PAGE: &str = include_str!("page.html");
/// The session cookie's name.
const SESSION_COOKIE: &str = "vlc_remote";
/// How long a connection may take to send its request or read the answer.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
/// Connections handled at once; past that, new ones are closed unanswered.
const MAX_CONNECTIONS: usize = 16;
/// How long stopping waits to wake the accepting thread.
const WAKE_TIMEOUT: Duration = Duration::from_secs(1);

/// What the server's threads share.
struct Shared {
    published: Published,
    commands: UnboundedSender<RemoteCommand>,
    password: PasswordSource,
    sessions: Mutex<Sessions>,
    lockout: Mutex<Lockout>,
    connections: AtomicUsize,
}

/// The running server; dropping it stops it and frees the port.
pub struct RemoteServer {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RemoteServer {
    /// Listens on `port` of every network interface (0 picks a free one), serving what
    /// `published` holds and sending commands to `commands`; phones sign in with what
    /// `password` gives. Fails when the port is taken.
    pub fn start(
        port: u16,
        published: Published,
        commands: UnboundedSender<RemoteCommand>,
        password: PasswordSource,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))?;
        let port = listener.local_addr()?.port();
        let shared = Arc::new(Shared {
            published,
            commands,
            password,
            sessions: Mutex::default(),
            lockout: Mutex::default(),
            connections: AtomicUsize::new(0),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = thread::Builder::new()
            .name("web-remote".into())
            .spawn(move || accept(&listener, &shared, &stopping))?;
        tracing::info!("web remote: listening on port {port}");
        Ok(Self { port, stop, thread: Some(thread) })
    }

    /// The port it listens on.
    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // A connection wakes the thread blocked accepting, which then sees it should stop.
        let local = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.port);
        TcpStream::connect_timeout(&local, WAKE_TIMEOUT).ok();
        if let Some(thread) = self.thread.take() {
            thread.join().ok();
        }
        tracing::info!("web remote: stopped");
    }
}

fn accept(listener: &TcpListener, shared: &Arc<Shared>, stop: &AtomicBool) {
    for stream in listener.incoming() {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let Ok(stream) = stream else { continue };
        if shared.connections.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
            shared.connections.fetch_sub(1, Ordering::SeqCst);
            continue;
        }
        let shared = shared.clone();
        let spawned = thread::Builder::new().name("web-remote-request".into()).spawn(move || {
            serve(&stream, &shared);
            shared.connections.fetch_sub(1, Ordering::SeqCst);
        });
        if let Err(error) = spawned {
            tracing::warn!("web remote: no thread for a connection: {error}");
        }
    }
}

/// Answers one connection's request.
fn serve(stream: &TcpStream, shared: &Shared) {
    let Ok(peer) = stream.peer_addr().map(|address| address.ip()) else { return };
    if !guard::is_local_peer(peer) {
        tracing::warn!("web remote: refused {peer}, which isn't on a local network");
        return;
    }
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    let response = match http::read_request(stream) {
        Ok(request) => respond(shared, &request, peer),
        Err(http::ReadError::Malformed) => Response::empty(Status::BadRequest),
        Err(http::ReadError::Io) => return,
    };
    response.write_to(stream).ok();
}

/// The answer to `request` from `peer`.
fn respond(shared: &Shared, request: &Request, peer: IpAddr) -> Response {
    if !guard::is_allowed_host(request) || !guard::is_same_origin(request) {
        return Response::error(Status::Forbidden, "Open the remote by the address VLC shows.");
    }
    let path = request.path.as_str();
    let signed_in = || shared.is_signed_in(request);
    match (request.method, path) {
        (Method::Get, "/") => Response::html(PAGE),
        (Method::Post, "/api/login") => shared.sign_in(request, peer),
        (_, "/api/state" | "/api/queue" | "/api/command") if !signed_in() => {
            Response::error(Status::Unauthorized, "Sign in first.")
        }
        (Method::Get, "/api/state") => Response::json(Status::Ok, shared.published.state_json()),
        (Method::Get, "/api/queue") => Response::json(Status::Ok, shared.published.queue_json()),
        (Method::Post, "/api/command") => shared.run(request),
        (_, "/" | "/api/login" | "/api/state" | "/api/queue" | "/api/command") => {
            Response::empty(Status::MethodNotAllowed)
        }
        _ => Response::empty(Status::NotFound),
    }
}

/// `POST /api/login`'s body.
#[derive(Deserialize)]
struct Login {
    password: String,
}

impl Shared {
    fn is_signed_in(&self, request: &Request) -> bool {
        let Some(token) = request.cookie(SESSION_COOKIE) else { return false };
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).contains(token)
    }

    fn sign_in(&self, request: &Request, peer: IpAddr) -> Response {
        let now = Instant::now();
        if self.lockout().is_locked(peer, now) {
            return Response::error(
                Status::TooManyRequests,
                "Too many wrong passwords. Try again in a minute.",
            );
        }
        let Ok(login) = serde_json::from_slice::<Login>(&request.body) else {
            return Response::empty(Status::BadRequest);
        };
        let saved = match (self.password)() {
            Ok(Some(saved)) if !saved.is_empty() => saved,
            Ok(_) => return Response::error(Status::Unavailable, NO_PASSWORD),
            Err(message) => return Response::error(Status::Unavailable, &message),
        };
        if !same_text(&saved, &login.password) {
            self.lockout().fail(peer, now);
            return Response::error(Status::Unauthorized, "Wrong password.");
        }
        self.lockout().succeed(peer);
        let Some(token) = Token::new() else {
            return Response::error(Status::Unavailable, "VLC couldn't start a session.");
        };
        let cookie =
            format!("{SESSION_COOKIE}={}; HttpOnly; SameSite=Strict; Path=/", token.as_str());
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner).add(token);
        Response::empty(Status::NoContent).with_cookie(cookie)
    }

    fn run(&self, request: &Request) -> Response {
        let Ok(command) = serde_json::from_slice::<RemoteCommand>(&request.body) else {
            return Response::empty(Status::BadRequest);
        };
        match self.commands.unbounded_send(command) {
            Ok(()) => Response::empty(Status::NoContent),
            Err(_) => Response::error(Status::Unavailable, "VLC's player window is closed."),
        }
    }

    fn lockout(&self) -> std::sync::MutexGuard<'_, Lockout> {
        self.lockout.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What signing in says while no password is saved.
const NO_PASSWORD: &str =
    "No password is set. Set one in VLC under Settings → Controls → Remotes first.";

#[cfg(test)]
mod tests;
