//! Extended M3U in UTF-8: `#EXTM3U`, then `#EXTINF:<seconds>,<title>` and the location of each
//! item. An unknown length is written as `-1`, as the format has it.

use std::fmt::Write as _;
use std::path::Path;

use url::Url;

use super::{Export, Item, PathStyle, relative_path};
use crate::app::media::Media;

/// The `.m3u8` text of `export`, with relative paths against `folder`.
pub fn render(export: &Export, folder: &Path) -> String {
    let mut text = format!("#EXTM3U\n#PLAYLIST:{}\n", one_line(export.title));
    for item in export.items {
        let seconds = item.duration.map_or(-1, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        // Writing to a `String` can't fail.
        let _ = writeln!(text, "#EXTINF:{seconds},{}", one_line(&item.title()));
        text.push_str(&location(item, export.paths, folder));
        text.push('\n');
    }
    text
}

/// Where `item` is, as one line of the playlist.
fn location(item: &Item, paths: PathStyle, folder: &Path) -> String {
    let path = match &item.media {
        Media::Url(url) => return url.clone(),
        Media::Device(device) => return device.mrl().to_owned(),
        Media::File(path) => path,
    };
    let relative = match paths {
        PathStyle::Relative => relative_path(path, folder),
        PathStyle::Absolute => None,
    };
    let written = relative.as_deref().unwrap_or(path).to_string_lossy().into_owned();
    if written.contains(['\n', '\r']) {
        // A line can't hold a line break; a `file://` URL can.
        return Url::from_file_path(path).map_or(one_line(&written), String::from);
    }
    if written.starts_with('#') {
        // Readers take a line starting with `#` for a comment.
        return format!("./{written}");
    }
    written
}

/// `text` with line breaks turned into spaces, so it stays on its line.
fn one_line(text: &str) -> String {
    text.replace(['\n', '\r'], " ")
}
