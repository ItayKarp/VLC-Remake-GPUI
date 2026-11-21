//! Opening what's on the clipboard (S1.3.3): file paths and URLs, one per line, as copied from
//! a file manager, a terminal or a browser's address bar.

use std::path::{Path, PathBuf};

use crate::app::media::{Media, has_url_scheme};

/// The paths and URLs in `text`, one per line. Quotes around a line are removed, `~/` means
/// the home folder, and lines that are neither a URL nor an absolute path are ignored: a
/// relative path has no folder to be relative to here.
pub fn items(text: &str, home: Option<&Path>) -> Vec<Media> {
    text.lines().filter_map(|line| item(line, home)).collect()
}

fn item(line: &str, home: Option<&Path>) -> Option<Media> {
    let line = unquote(line.trim());
    if line.is_empty() {
        return None;
    }
    if has_url_scheme(line) {
        return Some(Media::from_url(line));
    }
    let path = match line.strip_prefix("~/").or_else(|| line.strip_prefix("~\\")) {
        Some(rest) => home?.join(rest),
        None => PathBuf::from(line),
    };
    path.is_absolute().then_some(Media::File(path))
}

/// `text` without one pair of matching surrounding quotes.
fn unquote(text: &str) -> &str {
    ['"', '\'']
        .into_iter()
        .find_map(|quote| text.strip_prefix(quote)?.strip_suffix(quote))
        .map_or(text, str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_urls_and_absolute_paths_line_by_line() {
        let absolute = std::env::temp_dir().join("a b.mkv");
        let text = format!(
            "  https://example.com/live.m3u8\n\n\"{}\"\nrelative.mkv\nfile:///tmp/x%20y.mp4\n",
            absolute.display()
        );
        let mut expected =
            vec![Media::Url("https://example.com/live.m3u8".into()), Media::File(absolute)];
        expected.push(Media::from_url("file:///tmp/x%20y.mp4"));
        assert_eq!(items(&text, None), expected);
    }

    #[test]
    fn expands_the_home_folder() {
        let home = std::env::temp_dir();
        assert_eq!(
            items("'~/Videos/a.mkv'", Some(&home)),
            [Media::File(home.join("Videos/a.mkv"))]
        );
        assert_eq!(items("~/Videos/a.mkv", None), Vec::<Media>::new());
    }

    #[test]
    fn ignores_plain_text() {
        assert_eq!(items("hello world\nC:", None), Vec::<Media>::new());
        assert_eq!(items("", None), Vec::<Media>::new());
    }
}
