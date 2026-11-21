//! Online-only files (S4.2, edge case "cloud placeholders"): OneDrive Files On-Demand, iCloud
//! Drive and Dropbox keep a file's name and size on the disk and download its content when it is
//! opened. A scan that read them would download the whole library, so the indexer lists them
//! without opening them.
//!
//! - macOS marks them `SF_DATALESS` (File Provider: iCloud Drive, OneDrive, Dropbox).
//! - Windows marks them with the cloud files attributes (`RECALL_ON_DATA_ACCESS`,
//!   `RECALL_ON_OPEN`) or, for older sync clients, `OFFLINE`.
//! - Linux has no such files.

use std::fs::Metadata;

/// Whether `metadata` is an online-only file's.
#[cfg(target_os = "macos")]
pub fn is_online_only(metadata: &Metadata) -> bool {
    use std::os::macos::fs::MetadataExt as _;
    /// `<sys/stat.h>`: the file's content isn't on the disk.
    const SF_DATALESS: u32 = 0x4000_0000;
    metadata.st_flags() & SF_DATALESS != 0
}

/// Whether `metadata` is an online-only file's.
#[cfg(windows)]
pub fn is_online_only(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    const OFFLINE: u32 = 0x1000;
    const RECALL_ON_OPEN: u32 = 0x4_0000;
    const RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;
    metadata.file_attributes() & (OFFLINE | RECALL_ON_OPEN | RECALL_ON_DATA_ACCESS) != 0
}

/// Whether `metadata` is an online-only file's.
#[cfg(not(any(target_os = "macos", windows)))]
pub fn is_online_only(_: &Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    #[test]
    fn a_file_written_here_is_on_the_disk() {
        let path = scratch_dir().join("local.mp4");
        std::fs::write(&path, b"x").expect("write");
        assert!(!is_online_only(&std::fs::metadata(&path).expect("metadata")));
    }
}
