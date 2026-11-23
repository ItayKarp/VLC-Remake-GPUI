//! XSPF: an XML `<trackList>` of `<track>`s with a `<location>` URI, a `<title>` and a
//! `<duration>` in milliseconds when it is known.

use std::fmt::Write as _;
use std::path::Path;

use quick_xml::escape::escape;
use url::Url;

use super::{Export, Item, PathStyle, relative_path};
use crate::app::media::Media;

/// The `.xspf` text of `export`, with relative URIs against `folder`.
pub fn render(export: &Export, folder: &Path) -> String {
    let mut text = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\">\n",
    );
    // Writing to a `String` can't fail.
    let _ = writeln!(text, "  <title>{}</title>\n  <trackList>", escape(export.title));
    for item in export.items {
        text.push_str("    <track>\n");
        let _ =
            writeln!(text, "      <location>{}</location>", escape(location(item, export, folder)));
        let _ = writeln!(text, "      <title>{}</title>", escape(item.title()));
        if let Some(duration) = item.duration {
            let _ = writeln!(text, "      <duration>{}</duration>", duration.as_millis());
        }
        text.push_str("    </track>\n");
    }
    text.push_str("  </trackList>\n</playlist>\n");
    text
}

/// `item`'s location as a URI: relative to `folder`, or a `file://` URL.
fn location(item: &Item, export: &Export, folder: &Path) -> String {
    let path = match &item.media {
        Media::Url(url) => return url.clone(),
        Media::Device(device) => return device.mrl().to_owned(),
        Media::File(path) => path,
    };
    let Ok(absolute) = Url::from_file_path(path) else {
        return path.to_string_lossy().into_owned();
    };
    if export.paths == PathStyle::Relative && relative_path(path, folder).is_some() {
        let relative =
            Url::from_directory_path(folder).ok().and_then(|base| base.make_relative(&absolute));
        if let Some(relative) = relative {
            return relative;
        }
    }
    absolute.into()
}
