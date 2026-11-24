//! Reads gettext `.po` and `.pot` files: the entries' context, English text, plural English text
//! and translations. Comments, flags and obsolete (`#~`) entries are skipped.

/// One entry of a `.po` file.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// `msgctxt`, telling apart two meanings of the same English.
    pub context: Option<String>,
    /// `msgid`, the English text.
    pub id: String,
    /// `msgid_plural`, the English plural, for an entry with plural forms.
    pub plural: Option<String>,
    /// `msgstr`, or each `msgstr[n]` in order; empty strings where nothing is translated yet.
    pub translations: Vec<String>,
}

/// Why a `.po` file couldn't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    /// The 1-based line it stopped at.
    pub line: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unreadable line {} in a .po file", self.line)
    }
}

/// Which field the string lines that follow continue.
#[derive(Clone, Copy)]
enum Field {
    Context,
    Id,
    Plural,
    Translation(usize),
}

/// The entries of a `.po` file, without its header (the entry with an empty `msgid`).
pub fn parse(text: &str) -> Result<Vec<Entry>, ParseError> {
    let mut entries = Vec::new();
    let mut entry = Entry::default();
    let mut field = None;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let error = ParseError { line: index + 1 };
        if let Some(continued) = line.strip_prefix('"') {
            let target = field.ok_or_else(|| error.clone())?;
            push(&mut entry, target, &unquote(continued).ok_or(error)?);
            continue;
        }
        let (keyword, value) = line.split_once(' ').ok_or_else(|| error.clone())?;
        let next = keyword_field(keyword).ok_or_else(|| error.clone())?;
        let starts_entry = match next {
            Field::Context => true,
            Field::Id => !matches!(field, Some(Field::Context)),
            Field::Plural | Field::Translation(_) => false,
        };
        if starts_entry {
            let finished = std::mem::take(&mut entry);
            if !finished.id.is_empty() {
                entries.push(finished);
            }
        }
        let quoted = value.trim().strip_prefix('"').ok_or_else(|| error.clone())?;
        push(&mut entry, next, &unquote(quoted).ok_or(error)?);
        field = Some(next);
    }
    if !entry.id.is_empty() {
        entries.push(entry);
    }
    Ok(entries)
}

fn keyword_field(keyword: &str) -> Option<Field> {
    match keyword {
        "msgctxt" => Some(Field::Context),
        "msgid" => Some(Field::Id),
        "msgid_plural" => Some(Field::Plural),
        "msgstr" => Some(Field::Translation(0)),
        _ => {
            let index = keyword.strip_prefix("msgstr[")?.strip_suffix(']')?;
            index.parse().ok().map(Field::Translation)
        }
    }
}

fn push(entry: &mut Entry, field: Field, text: &str) {
    match field {
        Field::Context => entry.context.get_or_insert_default().push_str(text),
        Field::Id => entry.id.push_str(text),
        Field::Plural => entry.plural.get_or_insert_default().push_str(text),
        Field::Translation(index) => {
            if entry.translations.len() <= index {
                entry.translations.resize(index + 1, String::new());
            }
            entry.translations[index].push_str(text);
        }
    }
}

/// The text of a quoted string whose opening quote is already gone: `Play\n"` is "Play" and a
/// line break.
fn unquote(rest: &str) -> Option<String> {
    let body = rest.strip_suffix('"')?;
    let mut text = String::with_capacity(body.len());
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            text.push(c);
            continue;
        }
        text.push(match chars.next()? {
            'n' => '\n',
            't' => '\t',
            other => other,
        });
    }
    Some(text)
}

/// `text` as a quoted `.po` string, for writing the template.
#[cfg(test)]
pub fn quote(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\t' => quoted.push_str("\\t"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# A translator's comment.
msgid ""
msgstr ""
"Language: de\n"

#: src/main_window.rs
msgid "Play"
msgstr "Abspielen"

msgctxt "verb"
msgid "Open"
msgstr ""
"Öff"
"nen"

msgid "{count} item"
msgid_plural "{count} items"
msgstr[0] "{count} Element"
msgstr[1] "{count} Elemente"

msgid "Say \"hi\"\n"
msgstr "Sag \"hallo\"\n"
"#;

    #[test]
    fn entries_contexts_plurals_and_continued_lines_are_read() {
        let entries = parse(SAMPLE).expect("the sample parses");
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].id, "Play");
        assert_eq!(entries[0].translations, ["Abspielen"]);
        assert_eq!(entries[1].context.as_deref(), Some("verb"));
        assert_eq!(entries[1].translations, ["Öffnen"]);
        assert_eq!(entries[2].plural.as_deref(), Some("{count} items"));
        assert_eq!(entries[2].translations, ["{count} Element", "{count} Elemente"]);
        assert_eq!(entries[3].id, "Say \"hi\"\n");
    }

    #[test]
    fn quoting_round_trips() {
        let text = "A \"quote\", a \\ and\na line";
        let quoted = quote(text);
        assert_eq!(unquote(&quoted[1..]).as_deref(), Some(text));
    }

    #[test]
    fn a_broken_line_is_reported() {
        assert_eq!(parse("msgid \"a\"\nnonsense here\n"), Err(ParseError { line: 2 }));
        assert_eq!(parse("\"orphan\"\n"), Err(ParseError { line: 1 }));
    }
}
