//! Hardware decoding's fallback (S5.10.2): media VLC closed unexpectedly on while the GPU decoded
//! it decodes in software from then on, and a GPU and driver that crashed [`CRASHES_TO_BLOCK`]
//! times decodes nothing more. The engine keeps the marker that tells
//! ([`crate::engine::libvlc::hardware`]); this reads what the last run left, remembers it in the
//! local state, and hands the engine the policy.
//!
//! A run that ended in a Rust panic left a crash report, so its marker isn't blamed on the GPU.
//! Changing "Hardware-accelerated decoding" forgets everything, so Off and back to Automatic
//! tries the GPU again.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use gpui::App;

use crate::diagnostics::{self, crash, system};
use crate::engine::libvlc::hardware::{self, CrashedDecode, HardwarePolicy};
use crate::settings::{self, HardwareDecoding, LocalState, SettingsStore};

/// How many crashes turn hardware decoding off for a GPU and driver.
pub const CRASHES_TO_BLOCK: u32 = 2;
/// The most media remembered; the oldest is forgotten first.
const MAX_SOFTWARE_MEDIA: usize = 200;
/// The marker's file name, in the logs folder.
const MARKER_FILE: &str = "hardware-decoding";

/// What the main window tells about a crash this launch noted, until it does.
static NOTICE: Mutex<Option<String>> = Mutex::new(None);

/// Reads what the last run left, and keeps the engine's policy in step with the settings.
pub fn init(cx: &mut App) {
    let logs = diagnostics::logs_dir();
    let marker = logs.as_ref().map(|logs| logs.join(MARKER_FILE));
    let crashed = marker.as_deref().and_then(hardware::take_marker);
    let panicked = logs.as_deref().is_some_and(crash::has_pending);
    if let Some(crashed) = crashed.filter(|_| !panicked) {
        tracing::warn!("VLC closed while the GPU decoded video; it now decodes that in software");
        let gpu = crashed.gpu.clone();
        let name = display_name(&crashed.media);
        settings::update(cx, |settings| remember(&mut settings.local, crashed));
        let message = if is_blocked(&settings::get(cx).local, &gpu) {
            tr!("VLC closed unexpectedly while the graphics card decoded video again, so it no \
                 longer decodes with it. Settings → Performance can turn it back on.")
            .to_owned()
        } else {
            tr!(
                "VLC closed unexpectedly while the graphics card decoded “{name}”, so that file \
                 now decodes without it.",
                name
            )
        };
        *NOTICE.lock().unwrap_or_else(PoisonError::into_inner) = Some(message);
    }
    apply(marker.clone(), cx);
    let mut was = settings::get(cx).prefs.performance.hardware_decoding;
    cx.observe_global::<SettingsStore>(move |cx| {
        let now = settings::get(cx).prefs.performance.hardware_decoding;
        if now != was {
            was = now;
            settings::update(cx, |settings| forget(&mut settings.local));
        }
        apply(marker.clone(), cx);
    })
    .detach();
}

/// What to tell about a crash this launch noted, once.
pub fn take_notice() -> Option<String> {
    NOTICE.lock().unwrap_or_else(PoisonError::into_inner).take()
}

/// A media key's name for a message: a path's file name, or the URL.
fn display_name(media: &str) -> String {
    Path::new(media)
        .file_name()
        .filter(|_| !media.contains("://"))
        .map_or_else(|| media.to_owned(), |name| name.to_string_lossy().into_owned())
}

/// Says when hardware decoding is off for this GPU, or some media decodes in software, after
/// crashes; for the Hardware-accelerated decoding row.
pub fn hint(cx: &App) -> Option<String> {
    let settings = settings::get(cx);
    if settings.prefs.performance.hardware_decoding == HardwareDecoding::Off {
        return None;
    }
    let local = &settings.local;
    if is_blocked(local, system::gpu()) {
        return Some(
            tr!("Off for this graphics card: VLC closed unexpectedly while it decoded video. \
                 Choose Off, then Automatic, to try it again.")
            .to_owned(),
        );
    }
    match local.software_decoding.len() {
        0 => None,
        count => Some(trn!(
            "{count} file decodes in software: VLC closed unexpectedly while the graphics card \
             decoded it.",
            "{count} files decode in software: VLC closed unexpectedly while the graphics card \
             decoded them.",
            count
        )),
    }
}

/// Notes a crash: the media decodes in software, and the GPU's count goes up.
fn remember(local: &mut LocalState, crashed: CrashedDecode) {
    local.software_decoding.retain(|media| *media != crashed.media);
    local.software_decoding.push(crashed.media);
    let excess = local.software_decoding.len().saturating_sub(MAX_SOFTWARE_MEDIA);
    local.software_decoding.drain(..excess);
    *local.hardware_decoding_crashes.entry(crashed.gpu).or_default() += 1;
}

/// Forgets every crash.
fn forget(local: &mut LocalState) {
    local.software_decoding.clear();
    local.hardware_decoding_crashes.clear();
}

fn is_blocked(local: &LocalState, gpu: &str) -> bool {
    local.hardware_decoding_crashes.get(gpu).is_some_and(|crashes| *crashes >= CRASHES_TO_BLOCK)
}

/// Hands the engine the policy the local state holds.
fn apply(marker: Option<PathBuf>, cx: &App) {
    let local = &settings::get(cx).local;
    let blocked_gpus = local
        .hardware_decoding_crashes
        .iter()
        .filter(|(_, crashes)| **crashes >= CRASHES_TO_BLOCK)
        .map(|(gpu, _)| gpu.clone())
        .collect();
    hardware::set_policy(HardwarePolicy {
        software_media: local.software_decoding.iter().cloned().collect::<HashSet<_>>(),
        blocked_gpus,
        current_gpu: system::gpu,
        marker,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crash(gpu: &str, media: &str) -> CrashedDecode {
        CrashedDecode { gpu: gpu.to_owned(), media: media.to_owned() }
    }

    #[test]
    fn crashes_send_media_to_software_and_block_a_gpu_at_the_limit() {
        let mut local = LocalState::default();
        remember(&mut local, crash("GPU A", "/a.mkv"));
        assert_eq!(local.software_decoding, ["/a.mkv"]);
        assert!(!is_blocked(&local, "GPU A"));
        remember(&mut local, crash("GPU A", "/a.mkv"));
        assert_eq!(local.software_decoding, ["/a.mkv"], "remembered once");
        assert!(is_blocked(&local, "GPU A"));
        assert!(!is_blocked(&local, "GPU B"), "another driver is another GPU");
        forget(&mut local);
        assert!(local.software_decoding.is_empty() && !is_blocked(&local, "GPU A"));
    }

    #[test]
    fn the_oldest_media_is_forgotten_first() {
        let mut local = LocalState::default();
        for n in 0..=MAX_SOFTWARE_MEDIA {
            remember(&mut local, crash("GPU", &format!("/{n}.mkv")));
        }
        assert_eq!(local.software_decoding.len(), MAX_SOFTWARE_MEDIA);
        assert_eq!(local.software_decoding[0], "/1.mkv");
    }
}
