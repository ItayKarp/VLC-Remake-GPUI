//! Command-line arguments and the launch routing decision (S0.2.1, S0.2.5, S0.2.6).
//!
//! Only what S0.2 needs: `--reset-onboarding` and files to open. Full parsing
//! (`--fullscreen`, `--enqueue`, URLs, forwarding to a running instance) is S0.6.

use std::ffi::OsString;
use std::path::PathBuf;

pub const RESET_ONBOARDING: &str = "--reset-onboarding";
/// Opens the component kit gallery (S0.4), for design checks.
pub const COMPONENT_GALLERY: &str = "--component-gallery";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    /// Show onboarding even though it was completed.
    pub reset_onboarding: bool,
    /// Open the component gallery instead of the app's windows.
    pub component_gallery: bool,
    /// Files (or folders) the app was asked to open, e.g. by double-clicking a video.
    pub files: Vec<PathBuf>,
}

impl Launch {
    pub fn from_env() -> Self {
        Self::parse(std::env::args_os().skip(1))
    }

    /// Parses the arguments after the program name. Unknown flags are ignored with a
    /// warning; everything after `--` is a file.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Self {
        let mut launch = Launch::default();
        let mut only_files = false;
        for arg in args {
            if !only_files && let Some(flag) = arg.to_str().filter(|a| a.starts_with("--")) {
                match flag {
                    "--" => only_files = true,
                    RESET_ONBOARDING => launch.reset_onboarding = true,
                    COMPONENT_GALLERY => launch.component_gallery = true,
                    _ => log::warn!("ignoring unknown command-line option {flag}"),
                }
                continue;
            }
            if !arg.is_empty() {
                launch.files.push(PathBuf::from(arg));
            }
        }
        launch
    }
}

/// Which window the app opens at startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Onboarding,
    /// The component gallery (`--component-gallery`).
    Gallery,
    /// The main window, opening these files (may be empty).
    Main(Vec<PathBuf>),
}

pub fn route(onboarding_complete: bool, launch: &Launch) -> Route {
    if launch.component_gallery {
        return Route::Gallery;
    }
    if launch.reset_onboarding {
        if !launch.files.is_empty() {
            log::warn!("{RESET_ONBOARDING}: ignoring the files passed with it");
        }
        return Route::Onboarding;
    }
    // D4: a first launch triggered by opening a file plays it right away with the
    // default theme and runs onboarding on the next plain launch, because
    // `onboarding_complete` stays false.
    if onboarding_complete || !launch.files.is_empty() {
        Route::Main(launch.files.clone())
    } else {
        Route::Onboarding
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Launch {
        Launch::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_flags_and_files() {
        assert_eq!(parse(&[]), Launch::default());
        let launch = parse(&["--reset-onboarding", "a.mkv", "--bogus", "b c.mp4"]);
        assert!(launch.reset_onboarding);
        assert_eq!(launch.files, [PathBuf::from("a.mkv"), "b c.mp4".into()]);
    }

    #[test]
    fn double_dash_ends_flags() {
        let launch = parse(&["--", "--reset-onboarding", ""]);
        assert!(!launch.reset_onboarding);
        assert_eq!(launch.files, [PathBuf::from("--reset-onboarding")]);
    }

    #[cfg(windows)]
    #[test]
    fn keeps_non_unicode_paths() {
        use std::os::windows::ffi::OsStringExt;
        let odd = OsString::from_wide(&[0xD800, 'a' as u16]);
        let launch = Launch::parse([odd.clone()]);
        assert_eq!(launch.files, [PathBuf::from(odd)]);
    }

    #[test]
    fn routing() {
        let plain = Launch::default();
        let with_file = parse(&["movie.mkv"]);
        let reset = parse(&["--reset-onboarding", "movie.mkv"]);
        let main = |files: &[&str]| Route::Main(files.iter().map(PathBuf::from).collect());

        assert_eq!(route(false, &plain), Route::Onboarding);
        assert_eq!(route(true, &plain), main(&[]));
        assert_eq!(route(false, &with_file), main(&["movie.mkv"]));
        assert_eq!(route(true, &with_file), main(&["movie.mkv"]));
        assert_eq!(route(true, &reset), Route::Onboarding);
        assert_eq!(route(false, &reset), Route::Onboarding);
        assert_eq!(
            route(false, &parse(&["--component-gallery"])),
            Route::Gallery
        );
    }
}
