//! ASS and SSA: the `Dialogue:` lines of the `[Events]` section, laid out by its `Format:` line,
//! placed by their style's alignment (`[V4+ Styles]` or SSA's `[V4 Styles]`) unless an `\an`
//! override moves them. Drawn this way only while Embedded Sub Format is off, in the user's
//! style: italic, bold and underline overrides stay, fonts, colors, positions and effects go.

use std::collections::HashMap;

use super::markup::{self, Builder};
use super::{Cue, Placement, time};

/// The events' fields when the file has no `Format:` line for them.
const DEFAULT_EVENT_FORMAT: [&str; 10] =
    ["layer", "start", "end", "style", "name", "marginl", "marginr", "marginv", "effect", "text"];

/// Which section of the file a line is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    /// ASS styles, aligned on a numpad.
    Styles,
    /// SSA styles, aligned the older way.
    LegacyStyles,
    Events,
    Other,
}

/// The cues of an ASS or SSA file.
pub(super) fn parse(text: &str) -> Vec<Cue> {
    let mut section = Section::Other;
    let mut style_format: Vec<String> = Vec::new();
    let mut event_format: Vec<String> = DEFAULT_EVENT_FORMAT.map(str::to_owned).to_vec();
    let mut styles: HashMap<String, Placement> = HashMap::new();
    let mut cues = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            section = section_named(line);
            continue;
        }
        let Some((key, value)) = line.split_once(':') else { continue };
        match (section, key.trim()) {
            (Section::Styles | Section::LegacyStyles, "Format") => style_format = fields(value),
            (Section::Styles | Section::LegacyStyles, "Style") => {
                if let Some((name, placement)) = style(&style_format, value, section) {
                    styles.insert(name, placement);
                }
            }
            (Section::Events, "Format") => event_format = fields(value),
            (Section::Events, "Dialogue") => cues.extend(dialogue(&event_format, value, &styles)),
            _ => {}
        }
    }
    cues
}

fn section_named(line: &str) -> Section {
    match line.to_ascii_lowercase().as_str() {
        "[v4+ styles]" => Section::Styles,
        "[v4 styles]" => Section::LegacyStyles,
        "[events]" => Section::Events,
        _ => Section::Other,
    }
}

/// A `Format:` line's field names, lowercase.
fn fields(value: &str) -> Vec<String> {
    value.split(',').map(|field| field.trim().to_ascii_lowercase()).collect()
}

/// The field `name` of a line laid out by `format`; the last field takes the rest of the line,
/// since text holds commas.
fn field<'a>(format: &[String], value: &'a str, name: &str) -> Option<&'a str> {
    let index = format.iter().position(|field| field == name)?;
    value.splitn(format.len(), ',').nth(index)
}

/// A style's name and where its alignment places text.
fn style(format: &[String], value: &str, section: Section) -> Option<(String, Placement)> {
    let name = field(format, value, "name")?.trim().to_owned();
    let alignment: u32 = field(format, value, "alignment")?.trim().parse().ok()?;
    let placement = match section {
        Section::LegacyStyles => markup::legacy_alignment(alignment),
        _ => markup::numpad(alignment),
    };
    Some((name, placement))
}

/// The cue a `Dialogue:` line makes.
fn dialogue(format: &[String], value: &str, styles: &HashMap<String, Placement>) -> Option<Cue> {
    let start = time::parse(field(format, value, "start")?)?;
    let end = time::parse(field(format, value, "end")?)?;
    let style = field(format, value, "style").map(|style| style.trim().trim_start_matches('*'));
    let placement = style.and_then(|style| styles.get(style)).copied();
    let mut builder = Builder::placed(placement);
    text(&mut builder, field(format, value, "text")?);
    let (spans, placement) = builder.finish();
    Some(Cue { start, end, spans, placement: placement.unwrap_or_default() })
}

/// Reads dialogue text: override blocks, and the `\N` (line break), `\n` (a space unless the
/// script wraps by hand) and `\h` (non-breaking space) escapes.
fn text(builder: &mut Builder, text: &str) {
    let mut rest = text;
    while let Some(at) = rest.find(['{', '\\']) {
        builder.text(&rest[..at]);
        rest = &rest[at..];
        let (consumed, replacement) = if let Some(block) = rest.strip_prefix('{') {
            match block.find('}') {
                Some(end) => {
                    builder.overrides(&block[..end]);
                    (end + 2, "")
                }
                None => (1, "{"),
            }
        } else {
            match rest.as_bytes().get(1) {
                Some(b'N') => (2, "\n"),
                Some(b'n') => (2, " "),
                Some(b'h') => (2, "\u{A0}"),
                _ => (1, "\\"),
            }
        };
        builder.text(replacement);
        rest = &rest[consumed..];
    }
    builder.text(rest);
}
