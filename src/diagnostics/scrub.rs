//! Taking file names and folders out of a crash report (S9.6.2). A panic's message may hold
//! whatever path the failing code had in hand, often the media playing; each path becomes
//! `<path>` with its extension kept, which helps with format bugs and says nothing about the
//! user. Source locations (`…/player.rs:42:7`) stay: they name the app's code, not the user's
//! files, and a backtrace is little use without them.

/// What a path is replaced with.
const REDACTED: &str = "<path>";
/// The extension of the app's source files, whose locations are kept.
const SOURCE_EXTENSION: &str = "rs";
/// The longest extension kept; anything longer is more likely part of a name.
const MAX_EXTENSION_CHARS: usize = 5;
/// Characters a path may follow. Anything else before a `/` makes it a ratio or part of a word.
const OPENERS: &[char] = &['"', '\'', '`', '(', '[', '<', '=', ':', ','];
/// Characters that end a path not opened by a quote. Spaces don't: a file name may hold them, so
/// such a path runs to the end of its line.
const TERMINATORS: &[char] = &['"', '\'', '`', ',', ')', ']', '>', ';'];
/// Quotes that, opening a path, end it too, spaces and all.
const QUOTES: &[char] = &['"', '\'', '`'];

/// `text` with every path replaced by `<path>` and its extension, source locations aside.
pub fn paths(text: &str) -> String {
    let mut scrubbed = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous = None;
    while let Some(next) = rest.chars().next() {
        let (taken, shown) = if previous.is_none_or(opens_path) && starts_path(rest) {
            let end = path_end(rest, previous);
            (end, replacement(&rest[..end]))
        } else {
            (next.len_utf8(), next.to_string())
        };
        previous = rest[..taken].chars().next_back();
        scrubbed.push_str(&shown);
        rest = &rest[taken..];
    }
    scrubbed
}

fn opens_path(character: char) -> bool {
    character.is_whitespace() || OPENERS.contains(&character)
}

/// Whether `text` starts with an absolute path: `/a`, `~/a`, `\\server`, or `C:\a` or `C:/a`.
fn starts_path(text: &str) -> bool {
    let mut chars = text.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some('/'), Some(next), _) => !next.is_whitespace(),
        (Some('~'), Some('/' | '\\'), _) | (Some('\\'), Some('\\'), _) => true,
        (Some(drive), Some(':'), Some('/' | '\\')) => drive.is_ascii_alphabetic(),
        _ => false,
    }
}

/// Where the path at the start of `text` ends: at the quote that opened it, or else at the end
/// of the line or the first punctuation that can't be part of it.
fn path_end(text: &str, opener: Option<char>) -> usize {
    let end = match opener.filter(|opener| QUOTES.contains(opener)) {
        Some(quote) => text.find([quote, '\n']),
        None => text.find(|c: char| c == '\n' || TERMINATORS.contains(&c)),
    };
    end.unwrap_or(text.len())
}

/// What a path shows as: itself for a source location, otherwise `<path>` and its extension.
fn replacement(path: &str) -> String {
    let file = without_location(path);
    match extension(file) {
        Some(SOURCE_EXTENSION) => path.to_owned(),
        Some(extension) => format!("{REDACTED}.{extension}"),
        None => REDACTED.to_owned(),
    }
}

/// `path` without a trailing `:line` or `:line:column`.
fn without_location(path: &str) -> &str {
    let mut file = path;
    for _ in 0..2 {
        match file.rsplit_once(':') {
            Some((head, tail)) if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) => {
                file = head;
            }
            _ => break,
        }
    }
    file
}

/// The extension of the path's last part, if it looks like one.
fn extension(path: &str) -> Option<&str> {
    let name = path.rsplit(['/', '\\']).next()?;
    let (stem, extension) = name.rsplit_once('.')?;
    let plausible = !stem.is_empty()
        && (1..=MAX_EXTENSION_CHARS).contains(&extension.len())
        && extension.chars().all(|c| c.is_ascii_alphanumeric());
    plausible.then_some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_paths_go_whole_spaces_and_all() {
        let message = r#"couldn't open "/home/pat/Secret Films/a b.mkv": not found"#;
        assert_eq!(paths(message), r#"couldn't open "<path>.mkv": not found"#);
    }

    #[test]
    fn bare_home_windows_and_network_paths_go() {
        assert_eq!(paths("open ~/Videos/x.mp4\nnext"), "open <path>.mp4\nnext");
        assert_eq!(paths(r"at C:\Users\pat\clip.avi,"), "at <path>.avi,");
        assert_eq!(paths(r"share \\nas\films\private"), "share <path>");
        assert_eq!(paths("url=file:///Volumes/USB/x.flac"), "url=file:<path>.flac");
    }

    #[test]
    fn source_locations_and_other_slashes_stay() {
        let frame = "  at /rustc/abc/library/std/src/panicking.rs:665:5\n  at ~/vlc/src/main.rs:9";
        assert_eq!(paths(frame), frame);
        assert_eq!(paths("1/2 done, a / b, and/or"), "1/2 done, a / b, and/or");
        assert_eq!(paths("Message: index out of bounds"), "Message: index out of bounds");
    }

    #[test]
    fn only_plausible_extensions_are_kept() {
        assert_eq!(paths("/home/pat/.config/vlc"), "<path>");
        assert_eq!(paths("/home/pat/My Holiday.mkv failed"), "<path>");
        assert_eq!(paths("/films/Name.with.longtail"), "<path>");
    }
}
