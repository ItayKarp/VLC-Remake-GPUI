//! Collects every string marked for translation in `src/` into `assets/i18n/messages.pot`, the
//! template translators and the pseudo-locales work from, and fails while that file is stale.
//! `RVLC_UPDATE_POT=1 cargo test i18n::extract` writes it.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};

use super::po::quote;

/// The template, from the crate root.
const TEMPLATE: &str = "assets/i18n/messages.pot";
/// The variable that makes the test write the template instead of comparing it.
const UPDATE: &str = "RVLC_UPDATE_POT";
/// The marking macros, and how many string literals each takes (context or plural first).
const MACROS: [(&str, Shape); 4] = [
    ("tr!(", Shape::Text),
    ("tr_noop!(", Shape::Text),
    ("trc!(", Shape::Context),
    ("trn!(", Shape::Plural),
];

#[derive(Clone, Copy)]
enum Shape {
    Text,
    Context,
    Plural,
}

/// One message: its context and English, its English plural, and the files that use it.
#[derive(Default)]
struct Message {
    plural: Option<String>,
    files: Vec<String>,
}

/// The messages by (context, English), sorted so the template diffs well.
type Messages = BTreeMap<(Option<String>, String), Message>;

fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

/// Reads the string literal at the start of `text` (after whitespace): its value and the rest.
fn literal(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();
    if let Some(raw) = text.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let body = raw[hashes..].strip_prefix('"')?;
        let end = format!("\"{}", "#".repeat(hashes));
        let close = body.find(&end)?;
        return Some((body[..close].to_owned(), &body[close + end.len()..]));
    }
    let mut chars = text.strip_prefix('"')?.char_indices();
    let mut value = String::new();
    while let Some((index, c)) = chars.next() {
        match c {
            '"' => return Some((value, &text[index + 2..])),
            '\\' => escape(&mut chars, &mut value)?,
            other => value.push(other),
        }
    }
    None
}

/// Reads an escape after its backslash into `value`; a line continuation skips the newline
/// and the indentation after it.
fn escape(chars: &mut std::str::CharIndices<'_>, value: &mut String) -> Option<()> {
    match chars.next()?.1 {
        'n' => value.push('\n'),
        't' => value.push('\t'),
        'r' => value.push('\r'),
        '0' => value.push('\0'),
        '\n' => {
            let rest = chars.as_str();
            let skipped = rest.len() - rest.trim_start().len();
            for _ in 0..rest[..skipped].chars().count() {
                chars.next();
            }
        }
        'u' => {
            let rest = chars.as_str();
            let close = rest.find('}')?;
            let code = u32::from_str_radix(&rest[1..close], 16).ok()?;
            value.push(char::from_u32(code)?);
            for _ in 0..=close {
                chars.next();
            }
        }
        other => value.push(other),
    }
    Some(())
}

/// The literal after a comma at the start of `text`.
fn next_literal(text: &str) -> Option<(String, &str)> {
    literal(text.trim_start().strip_prefix(',')?)
}

fn collect(source: &str, file: &str, messages: &mut Messages) {
    for (name, shape) in MACROS {
        for (at, _) in source.match_indices(name) {
            let preceded_by_ident =
                source[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_');
            if preceded_by_ident || in_comment(source, at) {
                continue;
            }
            let Some((first, rest)) = literal(&source[at + name.len()..]) else { continue };
            let (key, plural) = match shape {
                Shape::Text => ((None, first), None),
                Shape::Context => match next_literal(rest) {
                    Some((text, _)) => ((Some(first), text), None),
                    None => continue,
                },
                Shape::Plural => ((None, first), next_literal(rest).map(|(plural, _)| plural)),
            };
            let message = messages.entry(key).or_default();
            message.plural = message.plural.take().or(plural);
            if !message.files.iter().any(|known| known == file) {
                message.files.push(file.to_owned());
            }
        }
    }
}

/// Whether `at` is in a `//` comment (where docs show the macros).
fn in_comment(source: &str, at: usize) -> bool {
    let line_start = source[..at].rfind('\n').map_or(0, |newline| newline + 1);
    source[line_start..at].trim_start().starts_with("//")
}

fn template(messages: &Messages) -> String {
    let mut out = String::from(
        "# The interface's text, for translators (S9.2). Generated by `RVLC_UPDATE_POT=1 cargo \
         test i18n::extract`: don't edit.\n# Plural forms are listed in CLDR's order of the \
         language's categories (zero, one, two, few, many, other).\nmsgid \"\"\nmsgstr \"\"\n\
         \"Content-Type: text/plain; charset=UTF-8\\n\"\n",
    );
    for ((context, text), message) in messages {
        out.push('\n');
        for file in &message.files {
            let _ = writeln!(out, "#: {file}");
        }
        if let Some(context) = context {
            let _ = writeln!(out, "msgctxt {}", quote(context));
        }
        let _ = writeln!(out, "msgid {}", quote(text));
        match &message.plural {
            Some(plural) => {
                let _ = writeln!(out, "msgid_plural {}", quote(plural));
                out.push_str("msgstr[0] \"\"\nmsgstr[1] \"\"\n");
            }
            None => out.push_str("msgstr \"\"\n"),
        }
    }
    out
}

fn extracted() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    files.sort();
    let mut messages = Messages::new();
    for path in files {
        let Ok(source) = fs::read_to_string(&path) else { continue };
        let relative =
            path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
        if relative.ends_with("i18n/extract.rs") || relative.contains("/tests") {
            continue;
        }
        collect(&source, &relative, &mut messages);
    }
    template(&messages)
}

#[test]
fn the_template_lists_every_marked_string() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(TEMPLATE);
    let fresh = extracted();
    if std::env::var_os(UPDATE).is_some() {
        fs::write(&path, &fresh).expect("the template can be written");
        return;
    }
    let saved = fs::read_to_string(&path).unwrap_or_default();
    assert!(
        saved == fresh,
        "{TEMPLATE} is stale: run `{UPDATE}=1 cargo test i18n::extract` and commit the result"
    );
}

#[test]
fn literals_escapes_and_every_macro_are_read() {
    let source = "tr!(\"Play\"); trc!(\"verb\", \"Open\"); trn!(\"{count} item\", \
                  \"{count} items\", n); x(tr_noop!(r#\"Say \"hi\"\"#)); tr!(\"a\\\n   b\\u{2026}\");\
                  attr!(\"no\")\n// tr!(\"doc\")";
    let mut messages = Messages::new();
    collect(source, "x.rs", &mut messages);
    let keys: Vec<_> =
        messages.keys().map(|(context, text)| (context.as_deref(), text.as_str())).collect();
    assert_eq!(
        keys,
        [
            (None, "Play"),
            (None, "Say \"hi\""),
            (None, "ab…"),
            (None, "{count} item"),
            (Some("verb"), "Open"),
        ]
    );
    let plural = &messages[&(None, "{count} item".to_owned())].plural;
    assert_eq!(plural.as_deref(), Some("{count} items"));
}
