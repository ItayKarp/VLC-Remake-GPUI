use std::net::{Ipv4Addr, SocketAddrV4};

use super::*;

fn target(protocol: Protocol, ip: Ipv4Addr) -> StreamTarget {
    let address = SocketAddr::V4(SocketAddrV4::new(ip, protocol.default_port()));
    StreamTarget {
        protocol,
        address,
        viewer_host: address.ip(),
        quality: StreamQuality::Medium,
        contents: Contents::Everything { subtitles: true },
        encoding: Encoding::default(),
        local: Local::PlaysHere,
    }
}

#[test]
fn http_plays_here_and_streams_a_transport_stream() {
    let options = target(Protocol::Http, Ipv4Addr::LOCALHOST).media_options();
    assert_eq!(
        options[0],
        ":sout=#duplicate{dst=display{delay=500},select=noaudio,dst=\"transcode{vcodec=h264,vb=3000,\
         maxheight=720,threads=2,soverlay,acodec=mp4a,ab=128,channels=2,samplerate=48000}:\
         std{access=http,mux=ts,dst=127.0.0.1:8080/stream}\"}"
    );
    assert_eq!(options[1..], [":no-sout-all", ":sout-x264-preset=medium"]);
}

#[test]
fn rtsp_and_udp_send_their_own_way() {
    let rtsp = target(Protocol::Rtsp, Ipv4Addr::UNSPECIFIED).chain();
    assert!(rtsp.ends_with(":rtp{sdp=rtsp://0.0.0.0:8554/stream}\"}"), "{rtsp}");
    let udp = target(Protocol::Udp, Ipv4Addr::new(192, 168, 1, 20)).chain();
    assert!(udp.ends_with(":std{access=udp,mux=ts,dst=192.168.1.20:1234}\"}"), "{udp}");
}

#[test]
fn streaming_only_leaves_the_screen_out() {
    let mut stream = target(Protocol::Http, Ipv4Addr::LOCALHOST);
    stream.local = Local::StreamOnly;
    assert!(stream.chain().starts_with("#duplicate{dst=\"transcode{"));
}

#[test]
fn what_is_left_out_is_kept_from_the_stream_only() {
    let mut stream = target(Protocol::Http, Ipv4Addr::LOCALHOST);
    stream.contents = Contents::SoundOnly;
    let chain = stream.chain();
    assert!(chain.ends_with(",select=audio}"), "{chain}");
    assert!(!chain.contains("vcodec") && !chain.contains("soverlay"), "{chain}");
    stream.contents = Contents::PictureOnly { subtitles: false };
    let chain = stream.chain();
    assert!(chain.ends_with(",select=video}") && !chain.contains("acodec"), "{chain}");
    stream.contents = Contents::Everything { subtitles: false };
    assert!(stream.chain().ends_with(",select=nospu}"));
}

#[test]
fn settings_send_sound_or_picture_or_nothing() {
    let mut recording = RecordingSettings { stream_audio: false, ..RecordingSettings::default() };
    assert_eq!(
        Contents::from_settings(&recording),
        Some(Contents::PictureOnly { subtitles: true })
    );
    recording.stream_video = false;
    assert_eq!(Contents::from_settings(&recording), None);
    recording.stream_audio = true;
    assert_eq!(Contents::from_settings(&recording), Some(Contents::SoundOnly));
}

#[test]
fn tuning_and_preset_reach_x264() {
    let mut stream = target(Protocol::Http, Ipv4Addr::LOCALHOST);
    stream.encoding = Encoding { preset: EncodingPreset::Ultrafast, tuning: Tuning::ZeroLatency };
    let options = stream.media_options();
    assert!(options.contains(&":sout-x264-preset=ultrafast".to_owned()));
    assert!(options.contains(&":sout-x264-tune=zerolatency".to_owned()));
}

#[test]
fn links_name_where_viewers_go() {
    let mut stream = target(Protocol::Http, Ipv4Addr::UNSPECIFIED);
    stream.viewer_host = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5));
    assert_eq!(stream.url(), "http://192.168.1.5:8080/stream");
    assert!(stream.is_open_to_network());
    let rtsp = target(Protocol::Rtsp, Ipv4Addr::LOCALHOST);
    assert_eq!(rtsp.url(), "rtsp://127.0.0.1:8554/stream");
    assert!(!rtsp.is_open_to_network());
    let udp = target(Protocol::Udp, Ipv4Addr::new(10, 0, 0, 2));
    assert_eq!(udp.url(), "udp://@:1234");
    assert!(!udp.is_open_to_network());
}
