//! Discs in Linux drives: the optical drives (`/dev/sr0`, …) and, for a mounted disc, where
//! `/proc/mounts` says it is. Audio CDs aren't mounted, so each drive is listed by itself too.

use std::fs;
use std::path::{Path, PathBuf};

use super::{Disc, DiscKind, kind_in};

/// Where the kernel lists block devices, and the prefix of optical drives' names.
const BLOCK_CLASS: &str = "/sys/class/block";
const OPTICAL_PREFIX: &str = "sr";
/// The mount table.
const MOUNTS: &str = "/proc/mounts";

/// The optical drives, each with where its disc is mounted, if it is.
pub fn discs() -> Vec<Disc> {
    let mounts = fs::read_to_string(MOUNTS).unwrap_or_default();
    let mut drives: Vec<String> = fs::read_dir(BLOCK_CLASS).map_or_default(|entries| {
        entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(OPTICAL_PREFIX))
            .collect()
    });
    drives.sort();
    drives.iter().map(|name| disc(name, &mounts)).collect()
}

/// The drive `name` (`sr0`) and its disc.
fn disc(name: &str, mounts: &str) -> Disc {
    let device = PathBuf::from(format!("/dev/{name}"));
    let mount = mount_of(&device, mounts);
    let kind =
        mount.as_deref().and_then(kind_in).or_else(|| mount.is_none().then_some(DiscKind::AudioCd));
    let label =
        mount.as_deref().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
    Disc {
        name: label.unwrap_or_else(|| format!("Drive {name}")),
        kind,
        device: Some(device),
        mount,
    }
}

/// Where `device` is mounted, from the mount table's lines (`device mount type …`, with spaces
/// in paths written as `\040`).
fn mount_of(device: &Path, mounts: &str) -> Option<PathBuf> {
    mounts.lines().find_map(|line| {
        let mut fields = line.split(' ');
        let from = fields.next()?;
        let to = fields.next()?;
        (Path::new(from) == device).then(|| PathBuf::from(to.replace("\\040", " ")))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounts_are_found_with_their_spaces() {
        let table = "/dev/sda1 / ext4 rw 0 0\n/dev/sr0 /media/me/MY\\040FILM udf ro 0 0\n";
        let found = mount_of(Path::new("/dev/sr0"), table);
        assert_eq!(found, Some(PathBuf::from("/media/me/MY FILM")));
        assert_eq!(mount_of(Path::new("/dev/sr1"), table), None);
    }
}
