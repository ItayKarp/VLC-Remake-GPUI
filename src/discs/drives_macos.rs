//! Discs in macOS drives: the volumes under `/Volumes` that are discs, each with the raw device
//! it was mounted from. macOS mounts an audio CD too, as `cddafs`, with a file per track.

use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use super::{Disc, DiscKind, kind_in};

/// Where macOS mounts volumes.
const VOLUMES: &str = "/Volumes";
/// The file systems discs mount as: audio CDs, data CDs, DVDs and Blu-rays.
const AUDIO_CD_FS: &str = "cddafs";
const DISC_FS: [&str; 2] = ["cd9660", "udf"];

/// The mounted discs.
pub fn discs() -> Vec<Disc> {
    let Ok(entries) = fs::read_dir(VOLUMES) else { return Vec::new() };
    let mut discs: Vec<Disc> = entries.flatten().filter_map(|entry| disc(&entry.path())).collect();
    discs.sort_by(|a, b| a.name.cmp(&b.name));
    discs
}

/// The disc mounted at `mount`, if it is one: an audio CD, a disc file system, or any volume
/// holding a disc's folders (a USB stick with a copied DVD).
fn disc(mount: &Path) -> Option<Disc> {
    let (fs_type, from) = mounted(mount)?;
    let kind = if fs_type == AUDIO_CD_FS { Some(DiscKind::AudioCd) } else { kind_in(mount) };
    if kind.is_none() && !DISC_FS.contains(&fs_type.as_str()) {
        return None;
    }
    let name = mount.file_name()?.to_string_lossy().into_owned();
    Some(Disc { name, kind, device: raw_device(&from), mount: Some(mount.to_owned()) })
}

/// The file system type and device the volume at `mount` was mounted from.
fn mounted(mount: &Path) -> Option<(String, String)> {
    let path = CString::new(mount.as_os_str().as_bytes()).ok()?;
    // SAFETY: `statfs` is plain old data, filled in by the call below.
    let mut info: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `path` is NUL-terminated and `info` is writable.
    if unsafe { libc::statfs(path.as_ptr(), &raw mut info) } != 0 {
        return None;
    }
    // SAFETY: the kernel NUL-terminates both names within their arrays.
    let text = |chars: &[libc::c_char]| unsafe { CStr::from_ptr(chars.as_ptr()) };
    let fs_type = text(&info.f_fstypename).to_string_lossy().into_owned();
    let from = text(&info.f_mntfromname).to_string_lossy().into_owned();
    Some((fs_type, from))
}

/// `/dev/rdisk4` for `/dev/disk4`: the raw device reads a disc without the buffer cache, as
/// libdvdcss and the CD reader want.
fn raw_device(from: &str) -> Option<PathBuf> {
    let disk = from.strip_prefix("/dev/disk")?;
    let whole = disk.split('s').next().unwrap_or(disk);
    Some(PathBuf::from(format!("/dev/rdisk{whole}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_devices_are_whole_disks() {
        assert_eq!(raw_device("/dev/disk4"), Some(PathBuf::from("/dev/rdisk4")));
        assert_eq!(raw_device("/dev/disk4s1"), Some(PathBuf::from("/dev/rdisk4")));
        assert_eq!(raw_device("map auto_home"), None);
    }

    #[test]
    fn the_start_up_volume_is_no_disc() {
        assert_eq!(disc(Path::new("/")), None);
    }
}
