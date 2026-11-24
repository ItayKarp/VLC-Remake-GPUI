//! Reading a podcast feed (S8.17.2): an RSS 2.0 `<channel>` of `<item>`s with an
//! `<enclosure url>`, or an Atom `<feed>` of `<entry>`s with a `<link rel="enclosure" href>`.
//! The iTunes tags (`itunes:duration`) are read by their local names.
//!
//! Feeds in the wild are often sloppy, so end tags aren't checked and what can't be read is
//! left out: an episode without a web link to play, a date or length that doesn't parse. A
//! document with neither a title nor an episode isn't a feed.

use std::borrow::Cow;

use quick_xml::Reader;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};

use super::dates;

/// What a feed says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Feed {
    /// The show's title.
    pub title: String,
    /// Its episodes, in the feed's order.
    pub episodes: Vec<Episode>,
}

/// One episode a feed lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Episode {
    /// What tells it apart from the feed's other episodes: its `guid` or `id`, else its link.
    pub guid: String,
    /// Its title.
    pub title: String,
    /// The media file to play, an `http(s)` link.
    pub url: String,
    /// When it was published, in seconds since the Unix epoch.
    pub published_s: Option<i64>,
    /// Its length in seconds.
    pub duration_s: Option<u32>,
}

/// Why a document isn't a feed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotAFeed;

/// What an episode is called when its feed gives it no title.
const UNTITLED: &str = "Untitled Episode";

/// The fields text can go into.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Field {
    ShowTitle,
    Title,
    Guid,
    Date,
    Duration,
}

/// The feed being read.
#[derive(Default)]
struct Parser {
    feed: Feed,
    /// Whether a `<channel>` or `<feed>` was seen.
    is_feed: bool,
    /// The episode being read, while inside an `<item>` or `<entry>`.
    episode: Option<Draft>,
    /// The field the next text goes into, and that text so far.
    field: Option<(Field, String)>,
    /// How deep the reader is.
    depth: usize,
    /// How deep the `<channel>` or `<feed>` is.
    channel_depth: Option<usize>,
}

/// An episode while it is read.
#[derive(Default)]
struct Draft {
    depth: usize,
    title: Option<String>,
    guid: Option<String>,
    url: Option<String>,
    date: Option<String>,
    duration: Option<String>,
}

/// Reads the feed `text` holds.
pub fn parse(text: &str) -> Result<Feed, NotAFeed> {
    let mut reader = Reader::from_str(text);
    let config = reader.config_mut();
    // Fields are trimmed whole: trimming each run would eat the spaces around `&amp;`.
    config.trim_text(false);
    config.check_end_names = false;
    config.allow_unmatched_ends = true;
    let mut parser = Parser::default();
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(start)) => parser.open(&start, false),
            Ok(Event::Empty(start)) => parser.open(&start, true),
            Ok(Event::End(_)) => parser.close(),
            Ok(Event::Text(text)) => parser.text(&unescaped(&String::from_utf8_lossy(&text))),
            Ok(Event::CData(data)) => parser.text(&String::from_utf8_lossy(&data)),
            Ok(Event::GeneralRef(reference)) => {
                parser.text(&unescaped(&format!("&{};", String::from_utf8_lossy(&reference))));
            }
            Ok(_) => {}
            Err(error) => {
                tracing::debug!("podcasts: XML error at {}: {error}", reader.error_position());
                break;
            }
        }
    }
    parser.finish()
}

impl Parser {
    fn open(&mut self, start: &BytesStart, empty: bool) {
        self.depth += 1;
        let name = lowercase(start.local_name().as_ref());
        let in_channel = self.channel_depth.is_some_and(|depth| depth + 1 == self.depth);
        match name.as_str() {
            "channel" | "feed" if self.channel_depth.is_none() => {
                self.is_feed = true;
                self.channel_depth = Some(self.depth);
            }
            "item" | "entry" if in_channel => {
                self.episode = Some(Draft { depth: self.depth, ..Draft::default() });
            }
            "title" if in_channel && self.feed.title.is_empty() => {
                self.field = Some((Field::ShowTitle, String::new()));
            }
            _ => self.open_in_episode(&name, start),
        }
        if empty {
            self.close();
        }
    }

    /// An element inside an episode: a field to read, or the link to play.
    fn open_in_episode(&mut self, name: &str, start: &BytesStart) {
        let Some(episode) = &mut self.episode else { return };
        if episode.depth + 1 != self.depth {
            return;
        }
        let field = match name {
            "title" if episode.title.is_none() => Field::Title,
            "guid" | "id" => Field::Guid,
            "pubdate" | "published" | "updated" if episode.date.is_none() => Field::Date,
            "duration" => Field::Duration,
            "enclosure" => {
                episode.url = episode.url.take().or_else(|| attribute(start, b"url"));
                return;
            }
            "link" if attribute(start, b"rel").as_deref() == Some("enclosure") => {
                episode.url = episode.url.take().or_else(|| attribute(start, b"href"));
                return;
            }
            _ => return,
        };
        self.field = Some((field, String::new()));
    }

    fn text(&mut self, text: &str) {
        if let Some((_, value)) = &mut self.field {
            value.push_str(text);
        }
    }

    fn close(&mut self) {
        if let Some((field, value)) = self.field.take() {
            self.store(field, value.trim().to_owned());
        }
        let depth = self.depth;
        if let Some(draft) = self.episode.take_if(|episode| episode.depth == depth) {
            self.feed.episodes.extend(draft.into_episode());
        }
        self.depth = self.depth.saturating_sub(1);
    }

    fn store(&mut self, field: Field, value: String) {
        if field == Field::ShowTitle {
            self.feed.title = value;
            return;
        }
        let Some(episode) = &mut self.episode else { return };
        let slot = match field {
            Field::Title => &mut episode.title,
            Field::Guid => &mut episode.guid,
            Field::Date => &mut episode.date,
            Field::Duration => &mut episode.duration,
            Field::ShowTitle => return,
        };
        *slot = Some(value);
    }

    fn finish(self) -> Result<Feed, NotAFeed> {
        let read = self.is_feed && !(self.feed.title.is_empty() && self.feed.episodes.is_empty());
        read.then_some(self.feed).ok_or(NotAFeed)
    }
}

impl Draft {
    /// The episode, if it has a web link to play.
    fn into_episode(self) -> Option<Episode> {
        let url = self.url.map(|url| url.trim().to_owned()).filter(|url| is_web_link(url))?;
        let title = self.title.filter(|title| !title.is_empty());
        let date = self.date.as_deref();
        Some(Episode {
            guid: self.guid.filter(|guid| !guid.is_empty()).unwrap_or_else(|| url.clone()),
            title: title.unwrap_or_else(|| UNTITLED.to_owned()),
            published_s: date
                .and_then(|date| dates::rfc2822(date).or_else(|| dates::rfc3339(date))),
            duration_s: self.duration.as_deref().and_then(dates::duration),
            url,
        })
    }
}

/// Whether `url` is an `http` or `https` link: a feed doesn't get to open local files.
pub fn is_web_link(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

fn lowercase(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_ascii_lowercase()
}

/// The element's attribute named `name` (any case), unescaped where possible.
fn attribute(start: &BytesStart, name: &[u8]) -> Option<String> {
    start.attributes().with_checks(false).flatten().find_map(|attribute| {
        let key = attribute.key.local_name();
        key.as_ref()
            .eq_ignore_ascii_case(name)
            .then(|| unescaped(&String::from_utf8_lossy(&attribute.value)))
    })
}

/// `raw` with XML escapes replaced, or as written when it has a stray `&`.
fn unescaped(raw: &str) -> String {
    unescape(raw).map_or_else(|_| raw.to_owned(), Cow::into_owned)
}

#[cfg(test)]
mod tests;
