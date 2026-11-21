//! Which media folders the library indexes (S4.2.1, S4.2.7): the folder onboarding picked and
//! any others in Settings, as video, and the OS Music folder, as music.
//!
//! **Decision D7:** the Music folder is added by itself, once, when it exists, so the Music
//! section isn't empty just because onboarding asked about videos. Removing it later sticks.

use std::path::{Path, PathBuf};

use super::indexer::Wanted;
use crate::db::roots::RootKind;
use crate::settings::LocalState;

/// The media folders `local` asks for, video first: none before onboarding is done.
pub fn wanted(local: &LocalState) -> Vec<Wanted> {
    if !local.onboarding_complete {
        return Vec::new();
    }
    let videos = local.media_folders.iter().map(|path| (path, RootKind::Video));
    let music = local.music_folders.iter().map(|path| (path, RootKind::Music));
    let mut wanted: Vec<Wanted> = Vec::new();
    for (path, kind) in videos.chain(music) {
        if !wanted.iter().any(|w| w.path == *path) {
            wanted.push(Wanted { path: path.clone(), kind });
        }
    }
    wanted
}

/// Whether the Music folder still has to be offered: onboarding is done and it wasn't yet.
pub fn should_offer_music(local: &LocalState) -> bool {
    local.onboarding_complete && !local.music_folder_added
}

/// Adds `music` (the OS Music folder, if it exists) to `local`'s music folders, once.
pub fn add_music_folder(local: &mut LocalState, music: Option<&Path>) {
    local.music_folder_added = true;
    let Some(music) = music else { return };
    let known = local.media_folders.iter().chain(&local.music_folders).any(|p| p == music);
    if !known {
        local.music_folders.push(music.to_path_buf());
    }
}

/// The OS Music folder, if it exists. Reads the disk.
pub fn music_folder() -> Option<PathBuf> {
    dirs::audio_dir().filter(|dir| dir.is_dir())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local() -> LocalState {
        LocalState {
            onboarding_complete: true,
            media_folders: vec!["/v".into()],
            ..LocalState::default()
        }
    }

    #[test]
    fn nothing_is_indexed_before_onboarding() {
        let local = LocalState { onboarding_complete: false, ..local() };
        assert_eq!(wanted(&local), []);
        assert!(!should_offer_music(&local));
    }

    #[test]
    fn videos_lead_and_duplicates_are_dropped() {
        let local = LocalState { music_folders: vec!["/m".into(), "/v".into()], ..local() };
        let found: Vec<(PathBuf, RootKind)> =
            wanted(&local).into_iter().map(|w| (w.path, w.kind)).collect();
        assert_eq!(found, [("/v".into(), RootKind::Video), ("/m".into(), RootKind::Music)]);
    }

    #[test]
    fn the_music_folder_is_offered_once() {
        let mut local = local();
        assert!(should_offer_music(&local));
        add_music_folder(&mut local, Some(Path::new("/m")));
        assert_eq!(local.music_folders, [PathBuf::from("/m")]);
        assert!(!should_offer_music(&local));
        // Removed later, it stays removed; a folder that's already a video folder isn't added.
        let mut other = LocalState { media_folders: vec!["/m".into()], ..self::local() };
        add_music_folder(&mut other, Some(Path::new("/m")));
        assert!(other.music_folders.is_empty() && other.music_folder_added);
        let mut none = self::local();
        add_music_folder(&mut none, None);
        assert!(none.music_folders.is_empty() && none.music_folder_added);
    }
}
