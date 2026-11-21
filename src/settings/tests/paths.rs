//! Where settings live, and how folder paths are stored in them.

use std::path::PathBuf;

use super::support::awkward_paths;
use crate::settings::paths::Locations;
use crate::settings::stored_path;

#[test]
fn config_dir_override() {
    let locations = Locations::resolve_with(Some("/portable/cfg".into())).unwrap();
    assert!(locations.prefs.ends_with("cfg/settings.toml"));
    assert!(locations.local.ends_with("cfg/local.toml"));
    // Empty means "not set".
    assert_ne!(Locations::resolve_with(Some("".into())), Some(locations));
}

#[test]
fn stored_path_round_trips() {
    for path in awkward_paths() {
        assert_eq!(stored_path::decode(&stored_path::encode(&path)), Some(path));
    }
    assert_eq!(stored_path::decode("os-path:garbage"), None);
}

#[cfg(windows)]
#[test]
fn non_unicode_windows_path_round_trips() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    // "C:\a" followed by an unpaired surrogate.
    let path = PathBuf::from(OsString::from_wide(&[0x43, 0x3A, 0x5C, 0x61, 0xD800]));
    assert!(path.to_str().is_none());
    let encoded = stored_path::encode(&path);
    assert!(encoded.starts_with("os-path:wide:"));
    assert_eq!(stored_path::decode(&encoded), Some(path));
}

#[cfg(unix)]
#[test]
fn non_utf8_unix_path_round_trips() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(OsString::from_vec(b"/tmp/\xff\xfe".to_vec()));
    let encoded = stored_path::encode(&path);
    assert!(encoded.starts_with("os-path:bytes:"));
    assert_eq!(stored_path::decode(&encoded), Some(path));
}
