//! Discs (S8.9.1): the DVDs, Blu-rays, audio CDs and Video CDs in the drives, a disc folder or
//! image picked by hand, and the media that plays one.
//!
//! A DVD plays from its drive's raw device where there is one, so libdvdcss can read it, with
//! its menus (`dvd://`) or without (`dvdsimple://`). A Blu-ray plays from where it is mounted,
//! with or without its menus (which need Java); libVLC ships no AACS keys, so an encrypted one
//! plays only where the user has installed libaacs and keys of their own. An audio CD plays as
//! one item per track, named from its CD-Text. A Video CD plays from its drive.

#[cfg(target_os = "linux")]
mod drives_linux;
#[cfg(target_os = "macos")]
mod drives_macos;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod drives_other;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use self::drives_linux as drives;
#[cfg(target_os = "macos")]
use self::drives_macos as drives;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use self::drives_other as drives;
use crate::app::media::{Device, Media};
use crate::engine::discs::CdTrack;

/// What kind of disc.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DiscKind {
    /// A DVD.
    Dvd,
    /// A Blu-ray.
    BluRay,
    /// An audio CD.
    AudioCd,
    /// A Video CD or Super Video CD.
    Vcd,
}

impl DiscKind {
    /// Every kind, as Open Disc lists them.
    pub const ALL: [Self; 4] = [Self::Dvd, Self::BluRay, Self::AudioCd, Self::Vcd];

    /// Its name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Dvd => "DVD",
            Self::BluRay => "Blu-ray",
            Self::AudioCd => "Audio CD",
            Self::Vcd => "Video CD",
        }
    }

    /// Whether it can have menus to play from.
    pub fn has_menus(self) -> bool {
        matches!(self, Self::Dvd | Self::BluRay)
    }

    /// Whether it has titles to start at.
    pub fn has_titles(self) -> bool {
        matches!(self, Self::Dvd | Self::BluRay | Self::Vcd)
    }
}

/// A disc in a drive, or a disc folder or image picked by hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disc {
    /// Its volume name, or the folder's or file's name.
    pub name: String,
    /// What it is, when that can be told from its files.
    pub kind: Option<DiscKind>,
    /// The drive's raw device (`/dev/rdisk4`, `/dev/sr0`, `D:`), for a disc in a drive.
    pub device: Option<PathBuf>,
    /// Where its files are: the mounted volume, or the folder or image picked.
    pub mount: Option<PathBuf>,
}

impl Disc {
    /// A disc folder or image the user picked.
    pub fn picked(path: &Path) -> Self {
        let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
        Self { name, kind: kind_in(path), device: None, mount: Some(path.to_owned()) }
    }

    /// Where libVLC reads it as `kind`: a DVD, a Video CD or an audio CD from the drive,
    /// a Blu-ray from its files.
    fn location(&self, kind: DiscKind) -> Option<&Path> {
        let (first, second) = match kind {
            DiscKind::BluRay => (&self.mount, &self.device),
            DiscKind::Dvd | DiscKind::Vcd | DiscKind::AudioCd => (&self.device, &self.mount),
        };
        first.as_deref().or(second.as_deref())
    }
}

/// The discs in the drives, found by their mounted files and, for audio CDs, which have no
/// files on some systems, by the drives themselves. Blocks briefly.
pub fn discs() -> Vec<Disc> {
    drives::discs()
}

/// What kind of disc the folder (or image) at `path` holds, by the folders each kind has; an
/// image (`.iso`) can't be told without opening it.
pub fn kind_in(path: &Path) -> Option<DiscKind> {
    let has = |name: &str| path.join(name).is_dir();
    if has("VIDEO_TS") || path.file_name().is_some_and(|name| name == "VIDEO_TS") {
        Some(DiscKind::Dvd)
    } else if has("BDMV") || path.file_name().is_some_and(|name| name == "BDMV") {
        Some(DiscKind::BluRay)
    } else if has("MPEGAV") || has("MPEG2") {
        Some(DiscKind::Vcd)
    } else {
        None
    }
}

/// How to play a disc.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Request<'a> {
    /// The disc.
    pub disc: &'a Disc,
    /// What to play it as.
    pub kind: DiscKind,
    /// Whether to start at its menus (DVD and Blu-ray).
    pub menus: bool,
    /// The title to start at, counted from 1; `None` for the start (or the menus).
    pub title: Option<u32>,
    /// The chapter of that title to start at, counted from 1.
    pub chapter: Option<u32>,
}

/// The media that plays `request`'s disc: one item, or one per track of an audio CD, whose
/// `tracks` were read first ([`crate::engine::discs::audio_cd_tracks`]). Empty when the disc
/// has nowhere to read from, or an audio CD has no tracks.
pub fn media(request: &Request, tracks: &[CdTrack]) -> Vec<Media> {
    let Some(base) = mrl(request) else { return Vec::new() };
    if request.kind == DiscKind::AudioCd {
        return audio_cd(&base, tracks);
    }
    let Some(device) = Device::new(&base) else { return Vec::new() };
    let device = match request.kind {
        DiscKind::BluRay if request.menus => device.with_option(":bluray-menu"),
        DiscKind::BluRay => device.with_option(":no-bluray-menu"),
        _ => device,
    };
    vec![Media::Device(device.titled(&request.disc.name))]
}

/// The MRL for audio CDs' track listing: the disc's drive, with no track.
pub fn audio_cd_mrl(disc: &Disc) -> Option<String> {
    let request =
        Request { disc, kind: DiscKind::AudioCd, menus: false, title: None, chapter: None };
    mrl(&request)
}

/// One item per track: the drive with the track's number, named by its CD-Text.
fn audio_cd(base: &str, tracks: &[CdTrack]) -> Vec<Media> {
    let base = base.split('#').next().unwrap_or(base);
    tracks
        .iter()
        .zip(1..)
        .filter_map(|(track, number): (&CdTrack, u32)| {
            let device = Device::new(base)?.with_option(format!(":cdda-track={number}"));
            let device = match &track.artist {
                Some(artist) => device.with_option(format!(":meta-artist={artist}")),
                None => device,
            };
            let title = track.title.clone().unwrap_or_else(|| format!("Track {number}"));
            Some(Media::Device(device.titled(&title)))
        })
        .collect()
}

/// libVLC's MRL for `request`: the scheme, the location as a URL path, and the title and
/// chapter to start at, which libVLC reads after a `#` for any disc.
fn mrl(request: &Request) -> Option<String> {
    let location = request.disc.location(request.kind)?;
    let scheme = match request.kind {
        DiscKind::Dvd if request.menus => "dvd",
        DiscKind::Dvd => "dvdsimple",
        DiscKind::BluRay => "bluray",
        DiscKind::AudioCd => "cdda",
        DiscKind::Vcd => "vcd",
    };
    let path = url::Url::from_file_path(location).ok()?;
    let path = path.as_str().strip_prefix("file://")?;
    let mut mrl = format!("{scheme}://{path}");
    if let Some(title) = request.title.filter(|_| request.kind.has_titles()) {
        let _ = write!(mrl, "#{title}");
        if let Some(chapter) = request.chapter {
            let _ = write!(mrl, ":{chapter}");
        }
    }
    Some(mrl)
}

#[cfg(test)]
mod tests;
