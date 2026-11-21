//! Command-line parsing (S0.6.2) and the launch routing decision (S0.2.1, S0.2.6).

use std::ffi::OsString;

use super::media::Media;
use super::request::{Launch, OpenRequest, Placement, StartScreen};

/// Runs onboarding again even though it was completed (S0.2.5).
pub const RESET_ONBOARDING: &str = "--reset-onboarding";
/// Opens the component kit gallery (S0.4), for design checks.
pub const COMPONENT_GALLERY: &str = "--component-gallery";
/// Shows the main window fullscreen.
pub const FULLSCREEN: &str = "--fullscreen";
/// Adds the items to the queue instead of replacing it.
pub const ENQUEUE: &str = "--enqueue";
/// macOS before 10.9 passed a process serial number to apps that Finder launched.
const MACOS_PSN_PREFIX: &str = "-psn_";

impl Launch {
    /// Parses this process's command line.
    pub fn from_env() -> Self {
        Self::parse(std::env::args_os().skip(1))
    }

    /// Parses the arguments after the program name. Unknown flags are ignored with a
    /// warning; everything after `--` is a file or URL. Relative paths are made
    /// absolute here, against this process's working directory.
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Self {
        let mut launch = Launch::default();
        let mut only_items = false;
        for arg in args {
            match arg.to_str().filter(|a| !only_items && is_flag(a)) {
                Some("--") => only_items = true,
                Some(flag) => launch.apply_flag(flag),
                None => launch.open.items.extend(Media::from_arg(&arg)),
            }
        }
        launch
    }

    fn apply_flag(&mut self, flag: &str) {
        match flag {
            RESET_ONBOARDING => self.start = StartScreen::Onboarding,
            COMPONENT_GALLERY => self.start = StartScreen::Gallery,
            FULLSCREEN => self.open.fullscreen = true,
            ENQUEUE => self.open.placement = Placement::Enqueue,
            _ if flag.starts_with(MACOS_PSN_PREFIX) => {}
            _ => log::warn!("ignoring unknown command-line option {flag}"),
        }
    }
}

/// Whether `arg` is an option rather than a file. Files whose names start with a
/// single `-` stay files.
fn is_flag(arg: &str) -> bool {
    arg.starts_with("--") || arg.starts_with(MACOS_PSN_PREFIX)
}

/// Which window the app opens at startup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// First-run setup (or `--reset-onboarding`).
    Onboarding,
    /// The component gallery (`--component-gallery`).
    Gallery,
    /// The main window, opening this (may be empty).
    Main(OpenRequest),
}

/// Decides which window opens first, from whether onboarding was completed and what
/// the command line asked for.
pub fn route(onboarding_complete: bool, launch: &Launch) -> Route {
    match launch.start {
        StartScreen::Gallery => Route::Gallery,
        StartScreen::Onboarding => {
            if !launch.open.items.is_empty() {
                log::warn!("{RESET_ONBOARDING}: ignoring the files passed with it");
            }
            Route::Onboarding
        }
        // D4: a first launch triggered by opening a file plays it right away with the
        // default theme and runs onboarding on the next plain launch, because
        // `onboarding_complete` stays false.
        StartScreen::Auto if onboarding_complete || !launch.open.items.is_empty() => {
            Route::Main(launch.open.clone())
        }
        StartScreen::Auto => Route::Onboarding,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn parse(args: &[&str]) -> Launch {
        Launch::parse(args.iter().map(OsString::from))
    }

    fn file(name: &str) -> Media {
        Media::File(std::path::absolute(name).expect("absolute path"))
    }

    #[test]
    fn parses_flags_and_items() {
        assert_eq!(parse(&[]), Launch::default());
        let launch = parse(&[
            "--reset-onboarding",
            "a.mkv",
            "--bogus",
            "b c.mp4",
            "-psn_0_1234",
            "https://example.com/live",
        ]);
        assert_eq!(launch.start, StartScreen::Onboarding);
        assert_eq!(
            launch.open.items,
            [file("a.mkv"), file("b c.mp4"), Media::Url("https://example.com/live".into())]
        );
        assert_eq!(launch.open.placement, Placement::Replace);
        assert!(!launch.open.fullscreen);
    }

    #[test]
    fn parses_fullscreen_and_enqueue() {
        let launch = parse(&["--fullscreen", "--enqueue", "a.mkv"]);
        assert_eq!(launch.start, StartScreen::Auto);
        assert!(launch.open.fullscreen);
        assert_eq!(launch.open.placement, Placement::Enqueue);
        assert_eq!(launch.open.items, [file("a.mkv")]);
    }

    #[test]
    fn double_dash_ends_flags() {
        let launch = parse(&["--", "--reset-onboarding", ""]);
        assert_eq!(launch.start, StartScreen::Auto);
        assert_eq!(launch.open.items, [file("--reset-onboarding")]);
    }

    #[cfg(windows)]
    #[test]
    fn keeps_non_unicode_paths() {
        use std::os::windows::ffi::OsStringExt;
        let odd = OsString::from_wide(&[0xD800, 'a' as u16]);
        let launch = Launch::parse([odd.clone()]);
        let expected = std::path::absolute(PathBuf::from(odd)).expect("absolute path");
        assert_eq!(launch.open.items, [Media::File(expected)]);
    }

    #[test]
    fn routing() {
        let plain = Launch::default();
        let with_file = parse(&["movie.mkv"]);
        let reset = parse(&["--reset-onboarding", "movie.mkv"]);
        let main = |items: Vec<Media>| Route::Main(OpenRequest::replace(items));

        assert_eq!(route(false, &plain), Route::Onboarding);
        assert_eq!(route(true, &plain), main(vec![]));
        assert_eq!(route(false, &with_file), main(vec![file("movie.mkv")]));
        assert_eq!(route(true, &with_file), main(vec![file("movie.mkv")]));
        assert_eq!(route(true, &reset), Route::Onboarding);
        assert_eq!(route(false, &reset), Route::Onboarding);
        assert_eq!(route(false, &parse(&["--component-gallery"])), Route::Gallery);
    }
}
