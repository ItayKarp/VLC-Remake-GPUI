//! Cue text markup: the HTML-like tags of SRT and WebVTT (`<i>`, `<b>`, `<u>`, `<font color>`),
//! the entities they escape characters with, and the ASS override blocks (`{\i1}`, `{\an8}`)
//! that ASS files are made of and SRT files borrow. Tags it doesn't draw (voices, classes, ruby,
//! karaoke, positions, fonts) are dropped and their text kept; drawings (`{\p1}`) are dropped
//! whole.

use super::{Emphasis, Placement, Span};

/// Characters the HTML-like formats escape, and what they stand for.
const ENTITIES: [(&str, &str); 8] = [
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&apos;", "'"),
    ("&nbsp;", "\u{A0}"),
    ("&lrm;", "\u{200E}"),
    ("&rlm;", "\u{200F}"),
];

/// The colors `<font color>` names most often, beside `#RRGGBB` and `#RGB`.
const COLOR_NAMES: [(&str, u32); 10] = [
    ("white", 0xFF_FFFF),
    ("black", 0),
    ("red", 0xFF_0000),
    ("lime", 0x00_FF00),
    ("green", 0x00_8000),
    ("blue", 0x00_00FF),
    ("yellow", 0xFF_FF00),
    ("cyan", 0x00_FFFF),
    ("magenta", 0xFF_00FF),
    ("gray", 0x80_8080),
];

/// Text read into styled runs, and where its override tags place it.
#[derive(Debug, Default)]
pub(super) struct Builder {
    spans: Vec<Span>,
    current: Span,
    colors: Vec<u32>,
    placement: Option<Placement>,
    /// Inside an ASS drawing (`{\p1}`), whose text is shapes, not words.
    drawing: bool,
}

impl Builder {
    /// A builder whose text starts at `placement` until an override tag moves it.
    pub(super) fn placed(placement: Option<Placement>) -> Self {
        Self { placement, ..Self::default() }
    }

    /// The runs read so far, and the placement the text asked for.
    pub(super) fn finish(mut self) -> (Vec<Span>, Option<Placement>) {
        self.flush();
        (self.spans, self.placement)
    }

    /// Adds plain text, without tags.
    pub(super) fn text(&mut self, text: &str) {
        if !self.drawing {
            self.current.text.push_str(text);
        }
    }

    /// Reads SRT or WebVTT cue text: HTML-like tags, entities, and ASS override blocks.
    pub(super) fn html(&mut self, text: &str) {
        let mut rest = text;
        while let Some(at) = rest.find(['<', '{', '&']) {
            self.text(&rest[..at]);
            rest = &rest[at..];
            rest = if let Some(after) = self.markup(rest) {
                after
            } else {
                self.text(&rest[..1]);
                &rest[1..]
            };
        }
        self.text(rest);
    }

    /// Reads the tag, block or entity `text` starts with, returning what follows it, or `None`
    /// when it is a literal character.
    fn markup<'a>(&mut self, text: &'a str) -> Option<&'a str> {
        if text.starts_with('&') {
            let (entity, meaning) = ENTITIES.iter().find(|(entity, _)| text.starts_with(entity))?;
            self.text(meaning);
            return Some(&text[entity.len()..]);
        }
        let (open, close) = if text.starts_with('<') { ('<', '>') } else { ('{', '}') };
        let end = text.find(close)?;
        let inner = &text[open.len_utf8()..end];
        // `1 < 2` is text: a tag starts right after its bracket and holds no other.
        if inner.contains(open) || inner.starts_with(char::is_whitespace) {
            return None;
        }
        if open == '<' {
            self.tag(inner);
        } else if inner.starts_with('\\') {
            self.overrides(inner);
        } else {
            return None;
        }
        Some(&text[end + 1..])
    }

    /// Applies an HTML-like tag's name and attributes: `i`, `/b`, `font color="red"`.
    fn tag(&mut self, tag: &str) {
        let (closing, tag) = match tag.strip_prefix('/') {
            Some(tag) => (true, tag),
            None => (false, tag),
        };
        let name = tag.split_whitespace().next().unwrap_or_default().to_ascii_lowercase();
        let emphasis = match name.as_str() {
            "i" => Emphasis::ITALIC,
            "b" => Emphasis::BOLD,
            "u" => Emphasis::UNDERLINE,
            "font" if closing => return self.pop_color(),
            "font" => return self.push_color(attribute(tag, "color").and_then(color)),
            _ => return,
        };
        self.emphasize(emphasis, !closing);
    }

    /// Applies an override block's tags, `\i1\an8`.
    pub(super) fn overrides(&mut self, block: &str) {
        for tag in block.split('\\').filter(|tag| !tag.is_empty()) {
            self.override_tag(tag.trim());
        }
    }

    fn override_tag(&mut self, tag: &str) {
        let number = |prefix: &str| tag.strip_prefix(prefix).and_then(|n| n.parse::<u32>().ok());
        if let Some(position) = number("an") {
            self.placement = Some(numpad(position));
        } else if let Some(position) = number("a") {
            self.placement = Some(legacy_alignment(position));
        } else if let Some(scale) = number("p") {
            self.flush();
            self.drawing = scale > 0;
        } else if let Some(on) = number("i") {
            self.emphasize(Emphasis::ITALIC, on > 0);
        } else if let Some(weight) = number("b") {
            self.emphasize(Emphasis::BOLD, weight > 0);
        } else if let Some(on) = number("u") {
            self.emphasize(Emphasis::UNDERLINE, on > 0);
        } else if tag == "r" {
            self.flush();
            self.current.emphasis = Emphasis::NONE;
        }
    }

    /// Turns `emphasis` on or off for the text that follows.
    pub(super) fn emphasize(&mut self, emphasis: Emphasis, on: bool) {
        let next = self.current.emphasis.with(emphasis, on);
        if next != self.current.emphasis {
            self.flush();
            self.current.emphasis = next;
        }
    }

    /// Colors the text that follows `color`, `0xRRGGBB`.
    pub(super) fn color(&mut self, color: u32) {
        self.push_color(Some(color));
    }

    fn push_color(&mut self, color: Option<u32>) {
        let color = color.or_else(|| self.colors.last().copied());
        self.flush();
        self.colors.extend(color);
        self.current.color = color;
    }

    fn pop_color(&mut self) {
        self.flush();
        self.colors.pop();
        self.current.color = self.colors.last().copied();
    }

    /// Ends the current run, so the next text starts one of its own.
    fn flush(&mut self) {
        if !self.current.text.is_empty() {
            let next = Span { text: String::new(), ..self.current.clone() };
            self.spans.push(std::mem::replace(&mut self.current, next));
        }
    }
}

/// Where ASS's numpad alignment (`\an`, V4+ styles) puts text: 1–3 bottom, 4–6 middle, 7–9 top.
pub(super) fn numpad(position: u32) -> Placement {
    match position {
        4..=6 => Placement::Middle,
        7..=9 => Placement::Top,
        _ => Placement::Bottom,
    }
}

/// Where SSA's alignment (`\a`, V4 styles) puts text: 1–3 bottom, 5–7 top, 9–11 middle.
pub(super) fn legacy_alignment(position: u32) -> Placement {
    match position {
        5..=7 => Placement::Top,
        9..=11 => Placement::Middle,
        _ => Placement::Bottom,
    }
}

/// The value of `name="value"`, `name='value'` or `name=value` among a tag's attributes.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let lower = tag.to_ascii_lowercase();
    let at = lower.find(&format!("{name}="))? + name.len() + 1;
    let value = &tag[at..];
    match value.chars().next()? {
        quote @ ('"' | '\'') => value[1..].split(quote).next(),
        _ => value.split_whitespace().next(),
    }
}

/// A color as `#RRGGBB`, `#RGB` or a common name.
fn color(value: &str) -> Option<u32> {
    let value = value.trim().to_ascii_lowercase();
    if let Some((_, color)) = COLOR_NAMES.iter().find(|(name, _)| *name == value) {
        return Some(*color);
    }
    let hex = value.strip_prefix('#').unwrap_or(&value);
    match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok(),
        3 => {
            let digits = u32::from_str_radix(hex, 16).ok()?;
            let channels = [digits >> 8, (digits >> 4) & 0xF, digits & 0xF];
            Some(channels.iter().fold(0, |color, channel| (color << 8) | (channel * 0x11)))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> (Vec<Span>, Option<Placement>) {
        let mut builder = Builder::default();
        builder.html(text);
        builder.finish()
    }

    fn span(text: &str, emphasis: Emphasis, color: Option<u32>) -> Span {
        Span { text: text.into(), emphasis, color }
    }

    #[test]
    fn tags_style_runs_and_entities_unescape() {
        let (spans, placement) = read("A <i>quiet</i> <font color=\"#F00\">r&amp;d</font>");
        assert_eq!(
            spans,
            [
                span("A ", Emphasis::NONE, None),
                span("quiet", Emphasis::ITALIC, None),
                span(" ", Emphasis::NONE, None),
                span("r&d", Emphasis::NONE, Some(0xFF_0000)),
            ]
        );
        assert_eq!(placement, None);
    }

    #[test]
    fn ass_overrides_in_srt_place_and_emphasize() {
        let (spans, placement) = read("{\\an8}{\\b1}Sign{\\b0} text");
        assert_eq!(placement, Some(Placement::Top));
        assert_eq!(spans[0], span("Sign", Emphasis::BOLD, None));
        assert_eq!(spans[1].text, " text");
    }

    #[test]
    fn unknown_tags_drop_and_stray_brackets_stay() {
        let (spans, _) = read("<v Roger>Hi</v> <c.loud>1 < 2</c> {not a tag} & co");
        let text: String = spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(text, "Hi 1 < 2 {not a tag} & co");
    }

    #[test]
    fn colors_read_as_hex_short_hex_and_names() {
        assert_eq!(color("#00ff00"), Some(0x00_FF00));
        assert_eq!(color("#0f0"), Some(0x00_FF00));
        assert_eq!(color("Yellow"), Some(0xFF_FF00));
        assert_eq!(color("chartreuse-ish"), None);
    }
}
