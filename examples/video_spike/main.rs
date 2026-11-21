//! S1.1 spike: plays a file with libVLC 3 and draws its video inside a GPUI window through
//! one of the presentation paths under test, with GPUI controls drawn on top. Prints
//! measurements once per second and a `RESULT` line at the end. Findings are in
//! `docs/adr/0001-playback-backend.md`.
//!
//! ```text
//! examples/video_spike/make_test_media.sh
//! cargo run --release --example video_spike -- --path surface target/spike-media/1080p60-h264.mp4
//! ```
//!
//! On macOS the libVLC runtime defaults to `vendor/libvlc/macos-arm64` (the `lib/`,
//! `plugins/` and `include/` folders of the official VLC .dmg); `--libvlc DIR` points elsewhere.

mod bgra;
mod libvlc;
mod options;
#[cfg(target_os = "macos")]
mod pixel_buffer;
mod player;
mod presenter;
mod sink;
mod slot;
mod stats;
mod track;
mod view;

use std::sync::Arc;

use crate::libvlc::{LibVlc, Runtime};
use crate::options::Options;

fn main() -> anyhow::Result<()> {
    let options = Options::parse(std::env::args().skip(1))?;
    let runtime = options
        .libvlc
        .as_deref()
        .map_or(Runtime { lib_dir: None, plugin_dir: None }, Runtime::under);
    if let Some(plugins) = &runtime.plugin_dir {
        // SAFETY: no other thread exists yet, so nothing reads the environment concurrently.
        unsafe { std::env::set_var("VLC_PLUGIN_PATH", plugins) };
    }
    let vlc = Arc::new(LibVlc::load(&runtime)?);
    println!("libVLC {}", vlc.version());
    gpui_platform::application().run(move |cx| {
        if let Err(error) = view::open(&options, vlc, cx) {
            eprintln!("{error:#}");
            cx.quit();
        }
    });
    Ok(())
}
