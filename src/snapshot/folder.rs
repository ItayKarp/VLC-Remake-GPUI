//! Where snapshots go: the folder Settings → Video names, else `Pictures/VLC`; and when that
//! can't be written (read-only, on a missing drive), the next place that can.

use std::path::PathBuf;

use crate::settings::expand_home;

/// The folder made inside the Pictures folder for snapshots.
const APP_FOLDER: &str = "VLC";

/// What the empty "Save snapshots to" field shows: the folder used then.
pub const DEFAULT_LABEL: &str =
    if cfg!(target_os = "windows") { r"Pictures\VLC" } else { "~/Pictures/VLC" };

/// The folder snapshots go to while the setting is empty: `VLC` in the OS's Pictures folder.
pub fn default_folder() -> PathBuf {
    let pictures =
        dirs::picture_dir().or_else(|| dirs::home_dir().map(|home| home.join("Pictures")));
    pictures.unwrap_or_else(std::env::temp_dir).join(APP_FOLDER)
}

/// The folder `setting` ("Save snapshots to", as typed) names.
pub fn chosen(setting: &str) -> PathBuf {
    let setting = setting.trim();
    if setting.is_empty() { default_folder() } else { expand_home(setting) }
}

/// Where to try saving, in order: the chosen folder, then the default one, the Pictures folder,
/// the home folder and the temporary folder, each once.
pub fn candidates(setting: &str) -> Vec<PathBuf> {
    let fallbacks = [Some(default_folder()), dirs::picture_dir(), dirs::home_dir()];
    let mut folders = vec![chosen(setting)];
    for folder in fallbacks.into_iter().flatten().chain([std::env::temp_dir()]) {
        if !folders.contains(&folder) {
            folders.push(folder);
        }
    }
    folders
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_setting_means_pictures_vlc() {
        assert_eq!(chosen("  "), default_folder());
        assert!(default_folder().ends_with(APP_FOLDER));
    }

    #[test]
    fn the_chosen_folder_comes_first_and_none_twice() {
        let folders = candidates("/somewhere/else");
        assert_eq!(folders[0], PathBuf::from("/somewhere/else"));
        assert_eq!(folders[1], default_folder());
        let mut unique = folders.clone();
        unique.dedup();
        assert_eq!(unique.len(), folders.len());
        assert_eq!(folders.last(), Some(&std::env::temp_dir()));
    }
}
