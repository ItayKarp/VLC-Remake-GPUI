//! The waveform cache (S7.8.2): a complete [`Peaks`] saved per media in the cache dir's
//! `waveforms` folder, named by the file's content identity (S1.7.2), so a moved or copied
//! file keeps it and a changed file is profiled again. Each file is a few hundred bytes, so
//! nothing is evicted; deleting the folder only costs the profiles being made again.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::profile::{BUCKETS, Peaks};
use crate::history::identity;

/// The cache's folder under the cache dir, and its files' extension.
const DIR_NAME: &str = "waveforms";
const EXTENSION: &str = "peaks";
/// What a cached profile starts with: a tag, then its format's version.
const HEADER: &[u8] = b"RVWF\x01";

/// Where `media`'s profile is cached; `None` without a cache dir or when the file can't be
/// read.
pub fn file_for(media: &Path) -> Option<PathBuf> {
    let size = fs::metadata(media).ok()?.len();
    let key = identity::file_key(media, size).ok()?;
    // The key names its hash before a colon, which some file systems don't allow in names.
    let name = key.rsplit(':').next().unwrap_or(&key);
    let dir = crate::settings::cache_dir()?.join(DIR_NAME);
    Some(dir.join(name).with_extension(EXTENSION))
}

/// The profile cached at `file`; `None` when there is none or it isn't one.
pub fn load(file: &Path) -> Option<Peaks> {
    let bytes = fs::read(file).ok()?;
    let levels = bytes.strip_prefix(HEADER)?;
    Peaks::complete(levels.to_vec())
}

/// Saves the complete `peaks` at `file`, through a temporary file so a reader never sees half
/// of it.
pub fn save(file: &Path, peaks: &Peaks) -> io::Result<()> {
    debug_assert!(peaks.is_complete() && peaks.levels().len() == BUCKETS);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = file.with_extension("part");
    fs::write(&partial, [HEADER, peaks.levels()].concat())?;
    fs::rename(&partial, file)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::engine::waveform::profile::Profile;

    #[test]
    fn a_saved_profile_loads_back_and_anything_else_is_refused() {
        let dir = std::env::temp_dir().join(format!("rvlc-waveform-{}", std::process::id()));
        let file = dir.join("media").with_extension(EXTENSION);
        let mut profile = Profile::new(Duration::from_secs(1));
        profile.push(&[0, 3000, -9000, 120].repeat(50), 1, 100);
        let peaks = profile.peaks(true);
        save(&file, &peaks).expect("saved");
        assert_eq!(load(&file), Some(peaks));
        fs::write(&file, b"not a profile").expect("written");
        assert_eq!(load(&file), None);
        assert_eq!(load(&dir.join("missing")), None);
        let _ = fs::remove_dir_all(dir);
    }
}
