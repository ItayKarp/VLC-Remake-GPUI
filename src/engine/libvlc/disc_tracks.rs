//! An audio CD's track list (S8.9.1), read with an instance of its own: libVLC opens a CD
//! without a track as a folder of tracks, named from the disc's CD-Text where it has one.

use std::ffi::{CString, c_int};
use std::sync::Arc;
use std::time::Duration;

use super::api::{LibVlc, Media};
use super::prepare::parse_with;
use crate::engine::discs::CdTrack;

/// The reader's instance never plays, so it needs no outputs.
const ARGUMENTS: [&str; 2] = ["--no-audio", "--no-video"];
/// `libvlc_media_parse_network`: parse what isn't a local file, as a CD isn't.
const PARSE_NETWORK: c_int = 0x01;
/// How long the drive may take to spin up and read the table of contents.
const READ_TIMEOUT: Duration = Duration::from_secs(20);
/// `libvlc_meta_Title` and `libvlc_meta_Artist`.
const META_TITLE: c_int = 0;
const META_ARTIST: c_int = 1;
/// Turns off looking the disc up in an online CD database, which would send its table of
/// contents away: the app asks for nothing online that Privacy doesn't allow.
const NO_CDDB: &std::ffi::CStr = c":cddb-server=";

/// The tracks of the audio CD at `mrl` (`cdda:///dev/rdisk4`), in order; empty when there is
/// no audio CD there or libVLC can't be loaded. Blocks while the drive reads.
pub fn audio_cd_tracks(mrl: &str) -> Vec<CdTrack> {
    let read = || -> anyhow::Result<Vec<CdTrack>> {
        let vlc = super::library()?;
        let instance = super::new_instance(&vlc, &ARGUMENTS)?;
        let tracks = read_tracks(&vlc, instance, mrl);
        // SAFETY: the instance is live, and nothing of it is used after this.
        unsafe { (vlc.api.libvlc_release)(instance) };
        Ok(tracks)
    };
    read().unwrap_or_else(|error| {
        tracing::warn!("discs: couldn't read the audio CD: {error:#}");
        Vec::new()
    })
}

/// Parses `mrl` as a folder and reads its items.
fn read_tracks(vlc: &Arc<LibVlc>, instance: *mut super::api::Instance, mrl: &str) -> Vec<CdTrack> {
    let Ok(text) = CString::new(mrl) else { return Vec::new() };
    let api = &vlc.api;
    // SAFETY: the instance is live and every string is NUL-terminated; the media is released
    // once below.
    unsafe {
        let media = (api.libvlc_media_new_location)(instance, text.as_ptr());
        if media.is_null() {
            return Vec::new();
        }
        (api.libvlc_media_add_option)(media, NO_CDDB.as_ptr());
        let tracks = if parse_with(vlc, media, PARSE_NETWORK, READ_TIMEOUT) {
            subitems(vlc, media)
        } else {
            Vec::new()
        };
        (api.libvlc_media_release)(media);
        tracks
    }
}

/// The items parsed `media` holds, each a track.
///
/// # Safety
///
/// `media` is a live, parsed media.
unsafe fn subitems(vlc: &LibVlc, media: *mut Media) -> Vec<CdTrack> {
    let api = &vlc.api;
    // SAFETY: the caller's promise; the list is held, locked while read, and released.
    unsafe {
        let list = (api.libvlc_media_subitems)(media);
        if list.is_null() {
            return Vec::new();
        }
        (api.libvlc_media_list_lock)(list);
        let count = (api.libvlc_media_list_count)(list).max(0);
        let tracks = (0..count)
            .filter_map(|index| {
                let item = (api.libvlc_media_list_item_at_index)(list, index);
                (!item.is_null()).then(|| {
                    let track = track(vlc, item);
                    (api.libvlc_media_release)(item);
                    track
                })
            })
            .collect();
        (api.libvlc_media_list_unlock)(list);
        (api.libvlc_media_list_release)(list);
        tracks
    }
}

/// What libVLC says of one track.
///
/// # Safety
///
/// `item` is a live media.
unsafe fn track(vlc: &LibVlc, item: *mut Media) -> CdTrack {
    // SAFETY: the caller's promise; `get_meta` returns a string we own, or NULL.
    let tag = |kind| unsafe { vlc.take_string((vlc.api.libvlc_media_get_meta)(item, kind)) };
    // SAFETY: the caller's promise.
    let millis = unsafe { (vlc.api.libvlc_media_get_duration)(item) };
    CdTrack {
        title: tag(META_TITLE),
        artist: tag(META_ARTIST),
        duration: u64::try_from(millis).ok().filter(|&ms| ms > 0).map(Duration::from_millis),
    }
}
