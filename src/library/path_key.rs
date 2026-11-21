//! Comparison keys for library paths (S4.1). Two paths are the same file when the file system
//! says so, not when their bytes match: Windows and default macOS volumes ignore case, and macOS
//! stores names decomposed (NFD) while dialogs, playlists and typing give them composed (NFC).
//!
//! A [`PathKey`] folds exactly what the root's file system folds, so lookups find the file
//! whatever form the path came in. The original path is stored next to it, untouched, for opening.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use icu_normalizer::ComposingNormalizerBorrowed;

use crate::settings::stored_path;

/// Whether a file system tells names apart by case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Case {
    /// `Movie.mkv` and `movie.mkv` are two files (Linux, case-sensitive APFS).
    Sensitive,
    /// `Movie.mkv` and `movie.mkv` are one file (Windows, default macOS volumes).
    Insensitive,
}

/// Whether the platform's file systems treat NFC and NFD names as one: APFS and HFS+ do, while
/// NTFS and the Linux file systems keep the code points they are given.
const FOLDS_NORMALIZATION: bool = cfg!(target_os = "macos");

impl Case {
    /// What the platform's usual file system does, when a folder can't be asked.
    pub const PLATFORM: Self =
        if cfg!(target_os = "linux") { Self::Sensitive } else { Self::Insensitive };

    /// Asks the file system holding the existing folder `dir`: whether its name with the case of
    /// every letter swapped finds the same folder. A name without letters (or a folder that
    /// can't be read) can't tell, and gets [`Case::PLATFORM`].
    pub fn detect(dir: &Path) -> Self {
        let Some(name) = dir.file_name().and_then(OsStr::to_str) else { return Self::PLATFORM };
        let swapped: String = name.chars().map(swap_case).collect();
        if swapped == name {
            return Self::PLATFORM;
        }
        let Ok(original) = fs::metadata(dir) else { return Self::PLATFORM };
        match fs::metadata(dir.with_file_name(swapped)) {
            Ok(twin) if is_same_file(&original, &twin) => Self::Insensitive,
            Ok(_) | Err(_) => Self::Sensitive,
        }
    }

    /// The database's `case_sensitive` column.
    pub fn is_sensitive(self) -> bool {
        self == Self::Sensitive
    }

    /// From the database's `case_sensitive` column.
    pub fn from_sensitive(sensitive: bool) -> Self {
        if sensitive { Self::Sensitive } else { Self::Insensitive }
    }
}

/// A path or name as the file system compares it: equal keys are the same file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PathKey(String);

impl PathKey {
    /// The key of `path` on a file system that treats case as `case`. A path that isn't valid
    /// Unicode is its own key, byte for byte, since no folding applies to it.
    pub fn new(path: &Path, case: Case) -> Self {
        let Some(text) = path.to_str() else { return Self(stored_path::encode(path)) };
        let text = if FOLDS_NORMALIZATION {
            ComposingNormalizerBorrowed::new_nfc().normalize(text)
        } else {
            text.into()
        };
        match case {
            Case::Sensitive => Self(text.into_owned()),
            Case::Insensitive => Self(text.to_lowercase()),
        }
    }

    /// The key as the database stores it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn swap_case(letter: char) -> char {
    let swapped: Vec<char> = if letter.is_uppercase() {
        letter.to_lowercase().collect()
    } else {
        letter.to_uppercase().collect()
    };
    // A letter whose other case is several letters (`ß`) is left alone: one swap is enough.
    match swapped[..] {
        [other] => other,
        _ => letter,
    }
}

#[cfg(unix)]
fn is_same_file(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    (a.dev(), a.ino()) == (b.dev(), b.ino())
}

/// Windows has no stable file identity in `std`; a swapped-case twin that exists is the same
/// folder unless both are folders created on a per-directory case-sensitive tree, which is rare.
#[cfg(not(unix))]
fn is_same_file(_: &fs::Metadata, _: &fs::Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::opening::scratch::scratch_dir;

    #[test]
    fn case_folds_only_where_the_file_system_does() {
        let path = Path::new("/Media/Movie.MKV");
        assert_eq!(PathKey::new(path, Case::Insensitive).as_str(), "/media/movie.mkv");
        assert_eq!(PathKey::new(path, Case::Sensitive).as_str(), "/Media/Movie.MKV");
    }

    #[test]
    fn composed_and_decomposed_names_meet_where_the_file_system_folds_them() {
        let composed = PathKey::new(Path::new("/m/Caf\u{e9}.mp3"), Case::Sensitive);
        let decomposed = PathKey::new(Path::new("/m/Cafe\u{301}.mp3"), Case::Sensitive);
        assert_eq!(composed == decomposed, FOLDS_NORMALIZATION);
    }

    #[cfg(unix)]
    #[test]
    fn a_path_that_isnt_unicode_is_its_own_key() {
        use std::os::unix::ffi::OsStrExt as _;
        let path = PathBuf::from(OsStr::from_bytes(b"/m/\xFFA.mkv"));
        let key = PathKey::new(&path, Case::Insensitive);
        assert_eq!(stored_path::decode(key.as_str()), Some(path));
    }

    #[test]
    fn detection_agrees_with_the_file_system() {
        let dir = scratch_dir().join("CaseProbe");
        fs::create_dir(&dir).expect("create");
        let insensitive = dir.with_file_name("cASEpROBE").exists();
        let expected = if insensitive { Case::Insensitive } else { Case::Sensitive };
        assert_eq!(Case::detect(&dir), expected);
    }

    #[test]
    fn a_name_without_letters_cant_tell() {
        let dir = scratch_dir().join("2024");
        fs::create_dir(&dir).expect("create");
        assert_eq!(Case::detect(&dir), Case::PLATFORM);
        assert_eq!(Case::detect(&dir.join("missing")), Case::PLATFORM);
    }
}
