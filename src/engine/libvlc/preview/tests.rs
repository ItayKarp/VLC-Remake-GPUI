//! Smoke tests of the preview decoder against a real libVLC and the S1.1 test media. Ignored by
//! default, like the backend's; run them with
//!
//! ```text
//! cargo test engine::libvlc -- --ignored --test-threads 1
//! ```

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use super::LibVlcPreview;
use crate::app::media::Media;
use crate::engine::preview::{self, PreviewDecoder, PreviewEvent, PreviewReceiver, PreviewRequest};

/// How long an answer may take.
const PATIENCE: Duration = Duration::from_secs(10);

fn decoder(name: &str) -> (LibVlcPreview, PreviewReceiver) {
    // SAFETY: these tests run one at a time (`--test-threads 1`), so no other thread reads the
    // environment meanwhile.
    unsafe { crate::engine::libvlc::runtime::prepare_environment() };
    let (sender, receiver) = preview::channel();
    let mut decoder = LibVlcPreview::new(sender).expect("libVLC");
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name);
    decoder.open(&Media::File(path));
    (decoder, receiver)
}

fn answer(receiver: &mut PreviewReceiver) -> PreviewEvent {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Ok(event) = receiver.try_recv() {
            return event;
        }
        assert!(Instant::now() < deadline, "no answer");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn pictures_at_different_times_differ_and_fit_the_card() {
    let (mut decoder, mut receiver) = decoder("1080p60-h264.mp4");
    let mut pictures = Vec::new();
    for (id, secs) in [(1, 2), (2, 7)] {
        let started = Instant::now();
        decoder.request(PreviewRequest { id, time: Duration::from_secs(secs) });
        let event = answer(&mut receiver);
        assert_eq!(event.id, id);
        let image = event.image.expect("a picture");
        // libVLC proposes the coded 1920×1088, so the width comes out a little under 480; the
        // card letterboxes by the player's aspect anyway.
        assert!(image.width <= preview::MAX_WIDTH && image.height == preview::MAX_HEIGHT);
        eprintln!("picture {id} took {:?}", started.elapsed());
        pictures.push(image.bgra);
    }
    assert_ne!(pictures[0], pictures[1], "both requests showed the same picture");
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn audio_only_media_answers_without_a_picture() {
    let (mut decoder, mut receiver) = decoder("audio-only.flac");
    decoder.request(PreviewRequest { id: 1, time: Duration::from_secs(1) });
    assert_eq!(answer(&mut receiver), PreviewEvent { id: 1, image: None });
}

/// How many bytes of `a` and `b` differ by more than a little.
fn difference(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).filter(|(a, b)| a.abs_diff(**b) > 16).count()
}

#[test]
#[ignore = "needs libVLC and the S1.1 test media"]
fn a_picture_shows_its_time_whatever_was_asked_before() {
    let picture = |times: &[u64]| {
        let (mut decoder, mut receiver) = decoder("1080p60-h264.mp4");
        let mut last = None;
        for (id, &secs) in (1..).zip(times) {
            decoder.request(PreviewRequest { id, time: Duration::from_secs(secs) });
            last = answer(&mut receiver).image;
        }
        last.expect("a picture").bgra
    };
    let (direct, elsewhere) = (picture(&[7]), picture(&[2]));
    let far = difference(&direct, &elsewhere);
    for before in [2, 9] {
        let near = difference(&direct, &picture(&[before, 7]));
        eprintln!("after {before} s: {near} bytes differ; 2 s against 7 s: {far}");
        // Within a frame or two of the direct picture, never a stale one from before.
        assert!(near * 4 < far, "after {before} s, the 7 s picture is off");
    }
}
