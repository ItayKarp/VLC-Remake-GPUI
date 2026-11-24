//! Smoke tests of streaming out (S8.12) against a real libVLC and the S1.1 test media, ignored
//! by default like [`super::tests`]. Each listens on loopback only, on a port found free.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcBackend;
use crate::app::media::Media;
use crate::engine::audio::AudioConfig;
use crate::engine::backend::{self, Backend, BackendEvent, EventReceiver};
use crate::engine::shutdown::{SHUTDOWN_TIMEOUT, Shutdown, shut_down};
use crate::engine::state::PlaybackState;
use crate::engine::stream_out::{Contents, Encoding, Local, Protocol, StreamQuality, StreamTarget};
use crate::engine::subtitle_style::SubtitleStyle;
use crate::engine::values::{Generation, Start, Volume};
use crate::settings::{EncodingPreset, Tuning};

/// How long any one expected event, or the stream's first bytes, may take.
const PATIENCE: Duration = Duration::from_secs(10);
/// An MPEG-TS packet's first byte.
const TS_SYNC: u8 = 0x47;

/// Waits for `wanted`, of any open; panics after [`PATIENCE`].
fn wait_for(receiver: &mut EventReceiver, wanted: &BackendEvent) {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        match receiver.try_recv() {
            Ok(tagged) if tagged.event == *wanted => return,
            Ok(_) => {}
            Err(_) => thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("timed out waiting for {wanted:?}");
}

/// A loopback address with a port nothing holds now.
fn free_address() -> SocketAddr {
    let probe = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    probe.local_addr().expect("its address")
}

/// A stream of `protocol` to `address`, only there, fast to encode.
fn target(protocol: Protocol, address: SocketAddr) -> StreamTarget {
    StreamTarget {
        protocol,
        address,
        viewer_host: address.ip(),
        quality: StreamQuality::Low,
        contents: Contents::Everything { subtitles: true },
        encoding: Encoding { preset: EncodingPreset::Ultrafast, tuning: Tuning::ZeroLatency },
        local: Local::StreamOnly,
    }
}

/// A backend, and the test media `name`.
fn backend(name: &str) -> (Box<LibVlcBackend>, EventReceiver, Media) {
    // SAFETY: these tests run one at a time (`--test-threads 1`).
    unsafe { super::runtime::prepare_environment() };
    let (events, receiver) = backend::channel();
    let mut backend = Box::new(
        LibVlcBackend::new(events, None, AudioConfig::default(), &SubtitleStyle::default())
            .expect("libVLC"),
    );
    backend.set_volume(Volume::new(0.));
    backend.set_mute(true);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name);
    (backend, receiver, Media::File(path))
}

/// Opens `media` streaming to `stream`, and waits for it to play.
fn open_streaming(
    backend: &mut LibVlcBackend,
    receiver: &mut EventReceiver,
    media: &Media,
    stream: StreamTarget,
) {
    let start = Start { stream: Some(stream), ..Start::default() };
    backend.open(media, start, Generation::default().next());
    wait_for(receiver, &BackendEvent::State(PlaybackState::Playing));
}

/// A connection to `address` once something listens there; panics after [`PATIENCE`].
fn connect(address: SocketAddr) -> TcpStream {
    let deadline = Instant::now() + PATIENCE;
    loop {
        match TcpStream::connect(address) {
            Ok(connection) => return connection,
            Err(error) if Instant::now() > deadline => panic!("nothing listens: {error}"),
            Err(_) => thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Asks the HTTP stream at `address` for its start: the reply's status line and the first
/// bytes after the headers.
fn fetch_http(address: SocketAddr) -> (String, Vec<u8>) {
    let mut connection = connect(address);
    connection.set_read_timeout(Some(PATIENCE)).unwrap();
    connection.write_all(b"GET /stream HTTP/1.0\r\n\r\n").unwrap();
    let mut received = Vec::new();
    let mut buffer = [0; 4096];
    while received.len() < 8192 {
        let read = connection.read(&mut buffer).expect("the stream sends");
        assert_ne!(read, 0, "the stream closed");
        received.extend_from_slice(&buffer[..read]);
    }
    let text = String::from_utf8_lossy(&received);
    let status = text.lines().next().unwrap_or_default().to_owned();
    let body_at = text.find("\r\n\r\n").expect("the headers end") + 4;
    (status, received[body_at..].to_vec())
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn http_sends_a_transport_stream() {
    let (mut backend, mut receiver, media) = backend("1080p60-h264.mp4");
    let address = free_address();
    open_streaming(&mut backend, &mut receiver, &media, target(Protocol::Http, address));
    let (status, body) = fetch_http(address);
    assert!(status.contains("200"), "{status}");
    assert!(body.contains(&TS_SYNC), "no transport stream packet");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn media_playing_starts_and_stops_streaming_where_it_is() {
    let (mut backend, mut receiver, media) = backend("1080p60-h264.mp4");
    backend.open(&media, Start::default(), Generation::default().next());
    wait_for(&mut receiver, &BackendEvent::State(PlaybackState::Playing));
    let address = free_address();
    backend.stream(Some(target(Protocol::Http, address)));
    let (status, _) = fetch_http(address);
    assert!(status.contains("200"), "{status}");
    backend.stream(None);
    let deadline = Instant::now() + PATIENCE;
    while TcpStream::connect(address).is_ok() {
        assert!(Instant::now() < deadline, "the stream still listens");
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn udp_sends_packets_to_its_address() {
    let (mut backend, mut receiver, media) = backend("1080p60-h264.mp4");
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    socket.set_read_timeout(Some(PATIENCE)).unwrap();
    let address = socket.local_addr().unwrap();
    open_streaming(&mut backend, &mut receiver, &media, target(Protocol::Udp, address));
    let mut packet = [0; 2048];
    let read = socket.recv(&mut packet).expect("a packet arrives");
    assert_eq!(packet.first(), Some(&TS_SYNC), "{read} bytes, not a transport stream");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn rtsp_describes_the_stream() {
    let (mut backend, mut receiver, media) = backend("1080p60-h264.mp4");
    let address = free_address();
    open_streaming(&mut backend, &mut receiver, &media, target(Protocol::Rtsp, address));
    // Until the encoders have their first pictures and sound there is nothing to describe, and
    // RTSP answers 500.
    let deadline = Instant::now() + PATIENCE;
    let mut reply = describe(address);
    while !reply.starts_with("RTSP/1.0 200") && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(200));
        reply = describe(address);
    }
    assert!(reply.starts_with("RTSP/1.0 200") && reply.contains("m=video"), "{reply}");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_taken_port_fails_the_stream_not_the_media() {
    let (mut backend, mut receiver, media) = backend("1080p60-h264.mp4");
    let taken = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("a free port");
    let address = taken.local_addr().unwrap();
    let start = Start { stream: Some(target(Protocol::Http, address)), ..Start::default() };
    backend.open(&media, start, Generation::default().next());
    wait_for(&mut receiver, &BackendEvent::StreamFailed);
    wait_for(&mut receiver, &BackendEvent::State(PlaybackState::Playing));
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}

/// RTSP's description of the stream at `address`, with its status line.
fn describe(address: SocketAddr) -> String {
    let mut connection = connect(address);
    connection.set_read_timeout(Some(PATIENCE)).unwrap();
    let request = format!("DESCRIBE rtsp://{address}/stream RTSP/1.0\r\nCSeq: 1\r\n\r\n");
    connection.write_all(request.as_bytes()).unwrap();
    // The headers and the description may come in separate reads.
    let mut reply = Vec::new();
    let mut chunk = [0; 4096];
    while !is_complete(&String::from_utf8_lossy(&reply)) {
        let read = connection.read(&mut chunk).expect("RTSP answers");
        if read == 0 {
            break;
        }
        reply.extend_from_slice(&chunk[..read]);
    }
    String::from_utf8_lossy(&reply).into_owned()
}

/// Whether an RTSP `reply` has its headers and as much body as they say.
fn is_complete(reply: &str) -> bool {
    let Some((headers, body)) = reply.split_once("\r\n\r\n") else { return false };
    let length = headers
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length:"))
        .and_then(|length| length.trim().parse::<usize>().ok());
    body.len() >= length.unwrap_or_default()
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn http_streams_while_playing_here() {
    let (mut backend, mut receiver, media) = backend("bars-709.mp4");
    let address = free_address();
    let stream = StreamTarget { local: Local::PlaysHere, ..target(Protocol::Http, address) };
    open_streaming(&mut backend, &mut receiver, &media, stream);
    let (status, body) = fetch_http(address);
    assert!(status.contains("200"), "{status}");
    assert!(body.contains(&TS_SYNC), "no transport stream packet");
    assert_eq!(shut_down(backend, SHUTDOWN_TIMEOUT), Shutdown::Clean);
}
