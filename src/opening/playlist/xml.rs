//! The XML playlist formats: XSPF and ASX.
//!
//! ASX files in the wild are often not well-formed (upper-case tags, unescaped `&` in URLs,
//! mismatched end tags), so tag and attribute names are matched case-insensitively, end tags
//! aren't checked, and a value that can't be unescaped is used as written.

use std::borrow::Cow;

use quick_xml::Reader;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesStart, Event};

use super::Parsed;
use super::text::Entry;

/// XSPF: the text of each `<location>` inside a `<track>`; the first one per track, since the
/// others are alternatives for the same track.
pub fn xspf(text: &str) -> Parsed {
    let mut entries = Vec::new();
    let mut path: Vec<String> = Vec::new();
    let mut location: Option<String> = None;
    let mut track_done = false;
    walk(text, |event| match event {
        Node::Open(name, _) => {
            if name == "track" {
                track_done = false;
            }
            if name == "location" && path.last().is_some_and(|p| p == "track") && !track_done {
                location = Some(String::new());
            }
            path.push(name);
        }
        Node::Text(text) => {
            if let Some(location) = &mut location {
                location.push_str(&text);
            }
        }
        Node::Close(name) => {
            if name == "location"
                && let Some(text) = location.take()
            {
                entries.push(Entry::Uri(text));
                track_done = true;
            }
            path.pop();
        }
    });
    Parsed::Entries(entries)
}

/// ASX: the first `<ref href>` of each `<entry>`, and each `<entryref href>` (a nested
/// playlist).
pub fn asx(text: &str) -> Parsed {
    let mut entries = Vec::new();
    let mut entry_has_ref = false;
    walk(text, |event| match event {
        Node::Open(name, href) => match name.as_str() {
            "entry" => entry_has_ref = false,
            "ref" if !entry_has_ref => {
                if let Some(href) = href {
                    entries.push(Entry::Location(href));
                    entry_has_ref = true;
                }
            }
            "entryref" => entries.extend(href.map(Entry::Location)),
            _ => {}
        },
        Node::Text(_) | Node::Close(_) => {}
    });
    Parsed::Entries(entries)
}

/// What [`walk`] reports: element names in lowercase, with their `href` attribute if any.
enum Node {
    Open(String, Option<String>),
    Text(String),
    Close(String),
}

/// Calls `visit` for each element and text run, until the end or the first syntax error (what
/// was read before the error is kept).
fn walk(text: &str, mut visit: impl FnMut(Node)) {
    let mut reader = Reader::from_str(text);
    let config = reader.config_mut();
    config.trim_text(true);
    config.check_end_names = false;
    config.allow_unmatched_ends = true;
    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) => return,
            Ok(event) => event,
            Err(error) => {
                tracing::debug!("playlist: XML error at {}: {error}", reader.error_position());
                return;
            }
        };
        match event {
            Event::Start(start) => visit(Node::Open(name(&start), href(&start))),
            Event::Empty(start) => {
                let name = name(&start);
                visit(Node::Open(name.clone(), href(&start)));
                visit(Node::Close(name));
            }
            Event::End(end) => {
                visit(Node::Close(lowercase(end.local_name().as_ref())));
            }
            Event::Text(text) => visit(Node::Text(unescaped(&String::from_utf8_lossy(&text)))),
            Event::CData(data) => visit(Node::Text(String::from_utf8_lossy(&data).into_owned())),
            Event::GeneralRef(reference) => {
                let name = String::from_utf8_lossy(&reference);
                visit(Node::Text(unescaped(&format!("&{name};"))));
            }
            _ => {}
        }
    }
}

fn name(start: &BytesStart) -> String {
    lowercase(start.local_name().as_ref())
}

fn lowercase(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_ascii_lowercase()
}

/// The element's `href` attribute (any case), unescaped where possible.
fn href(start: &BytesStart) -> Option<String> {
    start.attributes().with_checks(false).flatten().find_map(|attribute| {
        let key = attribute.key.local_name();
        if !key.as_ref().eq_ignore_ascii_case(b"href") {
            return None;
        }
        Some(unescaped(&String::from_utf8_lossy(&attribute.value)))
    })
}

/// `raw` with XML escapes replaced, or as written when it has a stray `&`.
fn unescaped(raw: &str) -> String {
    unescape(raw).map_or_else(|_| raw.to_owned(), Cow::into_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(parsed: Parsed) -> Vec<Entry> {
        let Parsed::Entries(entries) = parsed else { panic!("expected entries") };
        entries
    }

    #[test]
    fn xspf_takes_the_first_location_of_each_track() {
        let text = r#"<?xml version="1.0" encoding="UTF-8"?>
            <playlist version="1" xmlns="http://xspf.org/ns/0/">
              <location>ignored.xspf</location>
              <trackList>
                <track><title>A</title><location>file:///music/a%20b.mp3</location>
                  <location>http://mirror/a.mp3</location></track>
                <track><location>rel/R&amp;B.ogg</location></track>
                <track><location><![CDATA[c.flac]]></location></track>
              </trackList>
            </playlist>"#;
        assert_eq!(
            entries(xspf(text)),
            [
                Entry::Uri("file:///music/a%20b.mp3".into()),
                Entry::Uri("rel/R&B.ogg".into()),
                Entry::Uri("c.flac".into()),
            ]
        );
    }

    #[test]
    fn asx_tolerates_sloppy_files() {
        let text = r#"<ASX Version="3.0">
            <Entry><Title>One</Title><Ref HREF="http://a/x?b=1&c=2" /><REF href="http://fallback"/></Entry>
            <entry><ref href="local.wma"></entry>
            <EntryRef href="more.asx"/>
            </asx>"#;
        assert_eq!(
            entries(asx(text)),
            [
                Entry::Location("http://a/x?b=1&c=2".into()),
                Entry::Location("local.wma".into()),
                Entry::Location("more.asx".into()),
            ]
        );
    }

    #[test]
    fn broken_xml_keeps_what_was_read() {
        let text = "<asx><entry><ref href=\"a.mp3\"/></entry><entry><ref href=\"b.mp3";
        assert_eq!(entries(asx(text)), [Entry::Location("a.mp3".into())]);
    }
}
