use std::io::{Read, Write};

use futures::channel::mpsc::{UnboundedReceiver, unbounded};

use super::*;
use crate::remote::api::RemoteState;

const HOST: &str = "192.168.1.5:8080";
const PASSWORD: &str = "hunter2";

fn shared(password: Option<&'static str>) -> (Shared, UnboundedReceiver<RemoteCommand>) {
    let (commands, received) = unbounded();
    let shared = Shared {
        published: Published::default(),
        commands,
        password: Arc::new(move || Ok(password.map(str::to_owned))),
        sessions: Mutex::default(),
        lockout: Mutex::default(),
        connections: AtomicUsize::new(0),
    };
    (shared, received)
}

fn peer() -> IpAddr {
    "192.168.1.20".parse().unwrap()
}

fn request(method: &str, path: &str, extra: &str, body: &str) -> Request {
    let text = format!(
        "{method} {path} HTTP/1.1\r\nHost: {HOST}\r\nOrigin: http://{HOST}\r\n{extra}\
         Content-Length: {}\r\n\r\n{body}",
        body.len()
    );
    http::read_request(text.as_bytes()).unwrap()
}

/// Signs in and returns the `Cookie` header to send after.
fn sign_in(shared: &Shared) -> String {
    let response = respond(shared, &request("POST", "/api/login", "", PASSWORD_BODY), peer());
    assert_eq!(response.status, Status::NoContent);
    let mut out = Vec::new();
    response.write_to(&mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let cookie = text.lines().find_map(|line| line.strip_prefix("Set-Cookie: ")).unwrap();
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    format!("Cookie: {}\r\n", cookie.split(';').next().unwrap())
}

const PASSWORD_BODY: &str = r#"{"password":"hunter2"}"#;

#[test]
fn the_page_is_open_and_the_api_needs_signing_in() {
    let (shared, _) = shared(Some(PASSWORD));
    let page = respond(&shared, &request("GET", "/", "", ""), peer());
    assert_eq!(page.status, Status::Ok);
    for (method, path) in [("GET", "/api/state"), ("GET", "/api/queue"), ("POST", "/api/command")] {
        let response = respond(&shared, &request(method, path, "", "{}"), peer());
        assert_eq!(response.status, Status::Unauthorized, "{path}");
    }
    let forged = request("GET", "/api/state", "Cookie: vlc_remote=0123\r\n", "");
    assert_eq!(respond(&shared, &forged, peer()).status, Status::Unauthorized);
}

#[test]
fn a_signed_in_phone_reads_the_state_and_sends_commands() {
    let (shared, mut received) = shared(Some(PASSWORD));
    shared.published.set_state(RemoteState { title: "Film".into(), ..RemoteState::default() });
    let cookie = sign_in(&shared);
    let state = respond(&shared, &request("GET", "/api/state", &cookie, ""), peer());
    assert_eq!(state.status, Status::Ok);
    assert!(String::from_utf8(state.body).unwrap().contains(r#""title":"Film""#));
    let body = r#"{"command":"volume","percent":50}"#;
    let sent = respond(&shared, &request("POST", "/api/command", &cookie, body), peer());
    assert_eq!(sent.status, Status::NoContent);
    assert_eq!(received.try_recv().ok(), Some(RemoteCommand::Volume { percent: 50 }));
    let nonsense = respond(&shared, &request("POST", "/api/command", &cookie, "{}"), peer());
    assert_eq!(nonsense.status, Status::BadRequest);
}

#[test]
fn wrong_passwords_are_refused_then_shut_out() {
    let (shared, _) = shared(Some(PASSWORD));
    let wrong = request("POST", "/api/login", "", r#"{"password":"guess"}"#);
    for _ in 0..5 {
        assert_eq!(respond(&shared, &wrong, peer()).status, Status::Unauthorized);
    }
    let right = request("POST", "/api/login", "", PASSWORD_BODY);
    assert_eq!(respond(&shared, &right, peer()).status, Status::TooManyRequests);
}

#[test]
fn without_a_saved_password_nobody_signs_in() {
    for saved in [None, Some("")] {
        let (shared, _) = shared(saved);
        let login = request("POST", "/api/login", "", r#"{"password":""}"#);
        assert_eq!(respond(&shared, &login, peer()).status, Status::Unavailable);
    }
}

#[test]
fn other_sites_and_dns_names_are_refused() {
    let (shared, _) = shared(Some(PASSWORD));
    let text = format!(
        "POST /api/login HTTP/1.1\r\nHost: {HOST}\r\nOrigin: http://evil.example\r\n\
         Content-Length: {}\r\n\r\n{PASSWORD_BODY}",
        PASSWORD_BODY.len()
    );
    let cross_site = http::read_request(text.as_bytes()).unwrap();
    assert_eq!(respond(&shared, &cross_site, peer()).status, Status::Forbidden);
    let rebound = http::read_request(&b"GET / HTTP/1.1\r\nHost: evil.example:8080\r\n\r\n"[..]);
    assert_eq!(respond(&shared, &rebound.unwrap(), peer()).status, Status::Forbidden);
}

#[test]
fn serves_over_a_socket_and_frees_the_port_when_dropped() {
    let (commands, _received) = unbounded();
    let password: PasswordSource = Arc::new(|| Ok(Some(PASSWORD.to_owned())));
    let server = RemoteServer::start(0, Published::default(), commands, password).unwrap();
    let port = server.port();
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    write!(stream, "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\r\n").unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 200 OK"));
    assert!(answer.contains("<html"));
    drop(stream);
    drop(server);
    // Under parallel tests another socket may take the freed port for a moment.
    let freed = (0..40).any(|_| {
        let free = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port)).is_ok();
        if !free {
            thread::sleep(Duration::from_millis(50));
        }
        free
    });
    assert!(freed, "port {port} still held");
}

#[test]
fn a_taken_port_fails_to_start() {
    let held = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = held.local_addr().unwrap().port();
    let (commands, _received) = unbounded();
    let password: PasswordSource = Arc::new(|| Ok(None));
    assert!(RemoteServer::start(port, Published::default(), commands, password).is_err());
}
