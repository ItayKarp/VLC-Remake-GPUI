//! The HTTP proxy's **Test** button (Settings → Network, S5.10.1): connects to the saved proxy
//! and asks it `OPTIONS *`, which an HTTP server answers itself without fetching anything, so
//! the test reaches the proxy and nothing past it. The outcome shows under the HTTP proxy row
//! while the address is the one tested.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use gpui::{App, AppContext as _, Global, Window};

use crate::settings;

/// How long connecting, and then waiting for the answer, may each take.
const TIMEOUT: Duration = Duration::from_secs(5);
/// HTTP's "Proxy Authentication Required".
const PROXY_AUTHENTICATION_REQUIRED: u16 = 407;
/// The most of the answer read: its status line fits.
const ANSWER_BYTES: usize = 256;

/// The last test, for the address it tested.
#[derive(Default)]
struct ProxyTest {
    address: String,
    outcome: Option<Outcome>,
}

impl Global for ProxyTest {}

/// What a test found.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Outcome {
    /// There was no proxy address to test.
    NoAddress,
    /// Still connecting or waiting.
    Testing,
    /// An HTTP server answered.
    Answered,
    /// It answered that it wants a password.
    WantsPassword,
    /// Something answered, but not in HTTP.
    NotHttp,
    /// It couldn't be reached; why, in words.
    Unreachable(String),
}

impl Outcome {
    fn message(&self, host: &str) -> String {
        match self {
            Self::NoAddress => {
                tr!("Enter a proxy address to test. Empty, VLC uses the system's proxy.").to_owned()
            }
            Self::Testing => tr!("Testing {host}…", host),
            Self::Answered => tr!("{host} answered as an HTTP proxy.", host),
            Self::WantsPassword => {
                tr!("{host} answered and asks for a user name and password.", host)
            }
            Self::NotHttp => tr!("Something answered at {host}, but it isn't an HTTP proxy.", host),
            Self::Unreachable(why) => tr!("Couldn't reach {host}: {why}.", host, why),
        }
    }
}

/// "Test": tests the saved proxy address in the background.
pub fn run(_: &mut Window, cx: &mut App) {
    let address = settings::get(cx).prefs.network.http_proxy.clone();
    let target = settings::proxy_url(&address)
        .and_then(|url| Some((url.host_str()?.to_owned(), url.port_or_known_default()?)));
    let Some((host, port)) = target else {
        cx.set_global(ProxyTest { address, outcome: Some(Outcome::NoAddress) });
        cx.refresh_windows();
        return;
    };
    cx.set_global(ProxyTest { address: address.clone(), outcome: Some(Outcome::Testing) });
    cx.refresh_windows();
    let test = cx.background_spawn(async move { test(&host, port) });
    cx.spawn(async move |cx| {
        let outcome = test.await;
        cx.update(|cx| {
            if cx.default_global::<ProxyTest>().address == address {
                cx.set_global(ProxyTest { address, outcome: Some(outcome) });
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

/// The line under the HTTP proxy row: the last test's outcome, while the address is unchanged.
pub fn hint(cx: &App) -> Option<String> {
    let test = cx.try_global::<ProxyTest>()?;
    let address = &settings::get(cx).prefs.network.http_proxy;
    if test.address != *address {
        return None;
    }
    let outcome = test.outcome.as_ref()?;
    let shown = settings::proxy_url(address)
        .and_then(|url| {
            let host = url.host_str()?;
            Some(url.port().map_or_else(|| host.to_owned(), |port| format!("{host}:{port}")))
        })
        .unwrap_or_default();
    Some(outcome.message(&shown))
}

/// Connects to `host:port`, asks `OPTIONS *` and reads what comes back. Blocks.
fn test(host: &str, port: u16) -> Outcome {
    let addresses = match (host, port).to_socket_addrs() {
        Ok(addresses) => addresses.collect::<Vec<_>>(),
        Err(_) => return Outcome::Unreachable(tr!("no such host").to_owned()),
    };
    let mut last_error = tr!("no address for that host").to_owned();
    for address in addresses {
        match TcpStream::connect_timeout(&address, TIMEOUT) {
            Ok(stream) => return ask(stream, host),
            Err(error) => last_error = describe(&error),
        }
    }
    Outcome::Unreachable(last_error)
}

/// Asks the connected proxy `OPTIONS *` and judges its answer.
fn ask(mut stream: TcpStream, host: &str) -> Outcome {
    let request = format!("OPTIONS * HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    let sent = stream
        .set_read_timeout(Some(TIMEOUT))
        .and_then(|()| stream.set_write_timeout(Some(TIMEOUT)))
        .and_then(|()| stream.write_all(request.as_bytes()));
    if let Err(error) = sent {
        return Outcome::Unreachable(describe(&error));
    }
    let mut answer = [0; ANSWER_BYTES];
    match stream.read(&mut answer) {
        Ok(read) => judge(&String::from_utf8_lossy(&answer[..read])),
        Err(error) => Outcome::Unreachable(describe(&error)),
    }
}

/// What an answer's status line says about the server.
fn judge(answer: &str) -> Outcome {
    let mut status_line = answer.lines().next().unwrap_or_default().split_whitespace();
    let version = status_line.next().unwrap_or_default();
    if !version.starts_with("HTTP/") {
        return Outcome::NotHttp;
    }
    match status_line.next().and_then(|code| code.parse::<u16>().ok()) {
        Some(PROXY_AUTHENTICATION_REQUIRED) => Outcome::WantsPassword,
        Some(_) => Outcome::Answered,
        None => Outcome::NotHttp,
    }
}

/// An I/O error in the user's words.
fn describe(error: &std::io::Error) -> String {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::ConnectionRefused => tr!("nothing is listening on that port").to_owned(),
        ErrorKind::TimedOut | ErrorKind::WouldBlock => tr!("it didn't answer in time").to_owned(),
        ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => {
            tr!("it closed the connection").to_owned()
        }
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    /// A server on a free local port answering one connection with `answer`.
    fn serve(answer: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("its address").port();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("a connection");
            let mut request = [0; 512];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(answer.as_bytes());
        });
        port
    }

    #[test]
    fn an_http_server_answers_and_a_password_is_noticed() {
        let port = serve("HTTP/1.1 200 OK\r\nAllow: GET\r\n\r\n");
        assert_eq!(test("127.0.0.1", port), Outcome::Answered);
        let port = serve("HTTP/1.1 407 Proxy Authentication Required\r\n\r\n");
        assert_eq!(test("127.0.0.1", port), Outcome::WantsPassword);
    }

    #[test]
    fn something_else_is_not_a_proxy() {
        let port = serve("SSH-2.0-OpenSSH_9.6\r\n");
        assert_eq!(test("127.0.0.1", port), Outcome::NotHttp);
    }

    #[test]
    fn a_closed_port_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
        let port = listener.local_addr().expect("its address").port();
        drop(listener);
        assert!(matches!(test("127.0.0.1", port), Outcome::Unreachable(_)));
    }
}
