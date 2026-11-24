//! "Additional command line options" and "Collect playback statistics" (S5.3.4): libVLC options
//! the player's instance starts with.
//!
//! The text is split into options here, with quotes keeping spaces in a value, and handed to
//! `libvlc_new` as an argument list: no shell ever sees it. Each option must be `--name` or
//! `--name=value`. Options that would take over how VLC drives libVLC (its interfaces and video
//! output) are refused. When libVLC is loaded, [`check`] also asks it whether it knows them.

use std::sync::OnceLock;

use super::{ARGUMENTS, LIBRARY, new_instance};

/// Options that would start libVLC's own interfaces, replace the video output VLC draws with, or
/// make libVLC print and stop instead of starting.
const REFUSED: [&str; 11] = [
    "intf",
    "extraintf",
    "control",
    "vout",
    "video-title-show",
    "daemon",
    "one-instance",
    "help",
    "longhelp",
    "version",
    "list",
];
/// Turns libVLC's statistics off.
const NO_STATS: &str = "--no-stats";

/// The options the player's libVLC instance starts with, set once at startup.
static STARTUP: OnceLock<Vec<String>> = OnceLock::new();

/// Sets the options every player instance starts with; only the first call counts, since
/// these settings apply at the next launch.
pub fn set_startup(extra: &str, collect_statistics: bool) {
    let mut options = match parse(extra) {
        Ok(options) => options,
        Err(error) => {
            tracing::warn!("libvlc: ignoring the additional options: {error}");
            Vec::new()
        }
    };
    if !collect_statistics {
        options.push(NO_STATS.to_owned());
    }
    STARTUP.set(options).ok();
}

/// The options [`set_startup`] set.
pub(super) fn startup() -> &'static [String] {
    STARTUP.get().map_or(&[], Vec::as_slice)
}

/// Splits `text` into libVLC options, or says what is wrong with it.
pub fn parse(text: &str) -> Result<Vec<String>, String> {
    let options = split(text)?;
    for option in &options {
        check_syntax(option)?;
    }
    Ok(options)
}

/// Whether libVLC takes `options`: syntax first, then, when libVLC is loaded, a trial instance.
/// Without libVLC loaded only the syntax is checked; the player starts without options libVLC
/// refuses, and logs why.
pub fn check(text: &str) -> Result<(), String> {
    let options = parse(text)?;
    if options.is_empty() {
        return Ok(());
    }
    let Some(Ok(vlc)) = LIBRARY.get() else { return Ok(()) };
    let arguments: Vec<&str> =
        ARGUMENTS.iter().copied().chain(options.iter().map(String::as_str)).collect();
    match new_instance(vlc, &arguments) {
        Ok(instance) => {
            // SAFETY: the trial instance is live, unused and released once.
            unsafe { (vlc.api.libvlc_release)(instance) };
            Ok(())
        }
        Err(_) => Err(tr!("VLC doesn't know one of these options, or a value is wrong").into()),
    }
}

/// Splits on whitespace; double or single quotes keep spaces in a word and are removed.
fn split(text: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut quote: Option<char> = None;
    for c in text.chars() {
        if c.is_control() && !c.is_whitespace() {
            return Err(tr!("Options can't hold control characters").into());
        }
        match (quote, c) {
            (Some(open), c) if c == open => quote = None,
            (None, '"' | '\'') => {
                quote = Some(c);
                word.get_or_insert_default();
            }
            (None, c) if c.is_whitespace() => words.extend(word.take()),
            (_, c) => word.get_or_insert_default().push(c),
        }
    }
    if quote.is_some() {
        return Err(tr!("A quote isn't closed").into());
    }
    words.extend(word);
    Ok(words)
}

/// `--name` or `--name=value`, with a name libVLC could have and VLC lets the user set.
fn check_syntax(option: &str) -> Result<(), String> {
    let Some(rest) = option.strip_prefix("--") else {
        return Err(tr!("“{option}” isn't an option; options start with --", option));
    };
    let name = rest.split_once('=').map_or(rest, |(name, _)| name);
    let valid = name.starts_with(|c: char| c.is_ascii_alphanumeric())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    if !valid {
        return Err(tr!("“{option}” isn't an option name", option));
    }
    let base = name.strip_prefix("no-").unwrap_or(name);
    if REFUSED.contains(&base) {
        return Err(tr!("--{base} can't be changed here; VLC sets it itself", base));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_split_on_spaces_and_quotes_keep_values_together() {
        let parsed = parse(r#"--network-caching=3000  --sub-language="en fr" --no-osd"#);
        assert_eq!(parsed.unwrap(), ["--network-caching=3000", "--sub-language=en fr", "--no-osd"]);
        assert_eq!(parse("   ").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn mistakes_are_named() {
        assert_eq!(parse("-v").unwrap_err(), "“-v” isn't an option; options start with --");
        assert_eq!(parse("--a=\"b").unwrap_err(), "A quote isn't closed");
        assert_eq!(
            parse("--bad name!").unwrap_err(),
            "“name!” isn't an option; options start with --"
        );
        assert_eq!(parse("--x;rm").unwrap_err(), "“--x;rm” isn't an option name");
    }

    #[test]
    fn options_that_take_over_the_player_are_refused() {
        assert!(parse("--intf=qt").is_err());
        assert!(parse("--no-video-title-show").is_err());
        assert!(parse("--extraintf http").is_err());
    }

    #[test]
    fn shell_syntax_is_not_special() {
        assert_eq!(parse("--meta-title=$(whoami)").unwrap(), ["--meta-title=$(whoami)"]);
    }
}
