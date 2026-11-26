//! Smoke tests of converting (S8.10) against a real libVLC and the S1.1 test media, ignored by
//! default like [`super::tests`]. Each converts into a folder of its own in the temporary folder.

use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::app::media::Media;
use crate::convert::Preset;
use crate::engine::convert::{
    ConversionEvent, ConversionJob, ConversionReceiver, Failure, Request,
};

/// How long a conversion may take.
const PATIENCE: Duration = Duration::from_secs(60);

fn test_media(name: &str) -> Media {
    Media::File(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/spike-media").join(name))
}

/// An empty folder for one test's output.
fn out_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir().join(format!("rvlc-convert-{name}"));
    _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("a temporary folder");
    folder
}

fn start(source: Media, preset: Preset, output: &Path) -> (ConversionJob, ConversionReceiver) {
    // SAFETY: run on its own, as the other libVLC smoke tests.
    unsafe { super::runtime::prepare_environment() };
    let request = Request {
        source,
        options: preset.media_options(output),
        output: output.to_path_buf(),
        audio_only: preset.is_audio_only(),
    };
    ConversionJob::start(request)
}

/// The job's first report that isn't progress, after `on_progress` saw each progress report.
fn ending(events: &mut ConversionReceiver, mut on_progress: impl FnMut(f32)) -> ConversionEvent {
    let deadline = Instant::now() + PATIENCE;
    while Instant::now() < deadline {
        match events.try_recv() {
            Ok(ConversionEvent::Progress(fraction)) => on_progress(fraction),
            Ok(event) => return event,
            Err(_) => thread::sleep(Duration::from_millis(20)),
        }
    }
    panic!("timed out");
}

#[test]
#[ignore = "needs libVLC and the test media"]
fn a_video_converts_to_h264_mp4() {
    let output = out_folder("mp4").join("bars.mp4");
    let (_job, mut events) = start(test_media("bars-709.mp4"), Preset::Mp4H264, &output);
    assert_eq!(ending(&mut events, |_| {}), ConversionEvent::Finished(Ok(())));
    assert!(std::fs::metadata(&output).expect("the file").len() > 0);
}

#[test]
#[ignore = "needs libVLC and the test media"]
fn a_song_converts_to_mp3() {
    let output = out_folder("mp3").join("song.mp3");
    let (_job, mut events) = start(test_media("audio-only.flac"), Preset::Mp3, &output);
    assert_eq!(ending(&mut events, |_| {}), ConversionEvent::Finished(Ok(())));
    assert!(std::fs::metadata(&output).expect("the file").len() > 0);
}

#[test]
#[ignore = "needs libVLC and the test media"]
fn a_cancelled_conversion_leaves_no_file() {
    let output = out_folder("cancel").join("long.mp4");
    let (job, mut events) = start(test_media("1080p60-h264.mp4"), Preset::Mp4H265, &output);
    let mut cancelled = false;
    let event = ending(&mut events, |_| {
        job.cancel();
        cancelled = true;
    });
    assert!(cancelled, "it reported progress first");
    assert_eq!(event, ConversionEvent::Cancelled);
    assert!(!output.exists(), "the unfinished file is removed");
}

#[test]
#[ignore = "needs libVLC and the test media"]
fn a_picture_has_no_sound_to_convert() {
    let output = out_folder("nothing").join("frame.mp3");
    let (_job, mut events) = start(test_media("single-frame.png"), Preset::Mp3, &output);
    let failure = Failure::NothingToConvert { audio_only: true };
    assert_eq!(ending(&mut events, |_| {}), ConversionEvent::Finished(Err(failure)));
    assert!(!output.exists());
}

#[test]
#[ignore = "needs libVLC and the test media"]
fn a_file_that_exists_is_never_written_over() {
    let output = out_folder("exists").join("taken.mp3");
    std::fs::write(&output, b"mine").expect("a file");
    let (_job, mut events) = start(test_media("audio-only.flac"), Preset::Mp3, &output);
    assert!(matches!(
        ending(&mut events, |_| {}),
        ConversionEvent::Finished(Err(Failure::Unwritable(_)))
    ));
    assert_eq!(std::fs::read(&output).expect("still there"), b"mine");
}
