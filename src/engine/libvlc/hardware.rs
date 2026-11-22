//! Falling back to software decoding where hardware decoding crashed (S5.10.2).
//!
//! A buggy GPU driver takes the whole process down from inside libVLC, where nothing can catch
//! it. So while the GPU decodes video the worker keeps a marker file naming the GPU and the
//! media ([`decoding`]), and removes it once that stops. A marker the next launch finds means
//! the app died meanwhile; the app then decodes that media in software from then on, and turns
//! hardware decoding off for a GPU and driver that crashed more than once ([`set_policy`]).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use crate::app::media::Media as Source;

/// Where hardware decoding is used.
pub struct HardwarePolicy {
    /// Media that crashed while the GPU decoded it, by [`media_key`].
    pub software_media: HashSet<String>,
    /// GPUs (with their drivers) hardware decoding is off for.
    pub blocked_gpus: HashSet<String>,
    /// The GPU and driver the app renders with now.
    pub current_gpu: fn() -> &'static str,
    /// The marker file; `None` where the app has no place for files.
    pub marker: Option<PathBuf>,
}

/// What a marker the last run left behind names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrashedDecode {
    /// The GPU and driver.
    pub gpu: String,
    /// The media, by [`media_key`].
    pub media: String,
}

/// The policy [`set_policy`] set; without one, hardware decoding is used everywhere.
static POLICY: Mutex<Option<HardwarePolicy>> = Mutex::new(None);
/// The media the marker names now, so it is written once per media.
static MARKED: Mutex<Option<String>> = Mutex::new(None);

/// Sets where hardware decoding is used, replacing the previous policy.
pub fn set_policy(policy: HardwarePolicy) {
    *POLICY.lock().unwrap_or_else(PoisonError::into_inner) = Some(policy);
}

/// How the policy and the marker name `source`.
pub fn media_key(source: &Source) -> String {
    match source {
        Source::File(path) => path.to_string_lossy().into_owned(),
        Source::Url(url) => url.clone(),
    }
}

/// Reads and removes the marker at `path`: what was being decoded when the last run died.
pub fn take_marker(path: &Path) -> Option<CrashedDecode> {
    let text = fs::read_to_string(path).ok()?;
    fs::remove_file(path).ok();
    let (gpu, media) = text.split_once('\n')?;
    Some(CrashedDecode { gpu: gpu.to_owned(), media: media.trim_end_matches('\n').to_owned() })
}

/// Whether the GPU may decode `source`.
pub(super) fn allowed(source: &Source) -> bool {
    let policy = POLICY.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(policy) = policy.as_ref() else { return true };
    !policy.blocked_gpus.contains((policy.current_gpu)())
        && !policy.software_media.contains(&media_key(source))
}

/// Notes that the GPU now decodes `source`'s video, or, with `None`, that it stopped.
pub(super) fn decoding(source: Option<&Source>) {
    let key = source.map(media_key);
    let mut marked = MARKED.lock().unwrap_or_else(PoisonError::into_inner);
    if *marked == key {
        return;
    }
    let policy = POLICY.lock().unwrap_or_else(PoisonError::into_inner);
    let Some(policy) = policy.as_ref() else { return };
    let Some(path) = &policy.marker else { return };
    let written = match &key {
        Some(media) => fs::write(path, format!("{}\n{media}", (policy.current_gpu)())),
        None => fs::remove_file(path).or_else(|error| match error.kind() {
            std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(error),
        }),
    };
    match written {
        Ok(()) => *marked = key,
        Err(error) => tracing::warn!("libvlc: couldn't update the decoding marker: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_names_the_gpu_and_the_media_once() {
        let dir = std::env::temp_dir().join(format!("rvlc-hw-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("a temp folder");
        let marker = dir.join("hardware-decoding");
        set_policy(HardwarePolicy {
            software_media: HashSet::from(["/films/bad.mkv".to_owned()]),
            blocked_gpus: HashSet::new(),
            current_gpu: || "Test GPU (driver 1)",
            marker: Some(marker.clone()),
        });
        let good = Source::File("/films/good.mkv".into());
        assert!(allowed(&good));
        assert!(!allowed(&Source::File("/films/bad.mkv".into())));

        decoding(Some(&good));
        assert_eq!(
            fs::read_to_string(&marker).expect("written"),
            "Test GPU (driver 1)\n/films/good.mkv"
        );
        decoding(None);
        assert!(!marker.exists(), "stopping removes it");

        decoding(Some(&good));
        let crashed = take_marker(&marker).expect("left behind");
        assert_eq!(crashed.gpu, "Test GPU (driver 1)");
        assert_eq!(crashed.media, "/films/good.mkv");
        assert!(!marker.exists(), "taking it removes it");
        decoding(None);
        *POLICY.lock().unwrap_or_else(PoisonError::into_inner) = None;
        fs::remove_dir_all(&dir).ok();
    }
}
