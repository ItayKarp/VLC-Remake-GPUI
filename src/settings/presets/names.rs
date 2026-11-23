//! Preset names: never empty, never Default Preset's, never two alike. A taken name gets the
//! next free number, "Anime (2)".

use crate::settings::Preset;

/// Default Preset's name, which no custom preset may take.
pub const DEFAULT: &str = "Default Preset";
/// The name a new preset starts with.
pub const NEW: &str = "New Preset";
/// The longest name kept, in characters; the list truncates long ones on screen.
pub const MAX_CHARS: usize = 60;

/// `wanted`, trimmed and shortened, or with the first free number after it when another preset
/// than `except` (an index in `list`) has it. Names compare ignoring case.
pub fn unique(list: &[Preset], wanted: &str, except: Option<usize>) -> String {
    let others = list.iter().enumerate().filter(|(index, _)| Some(*index) != except);
    let others: Vec<&str> = others.map(|(_, preset)| preset.name.as_str()).collect();
    let taken = |name: &str| {
        name.eq_ignore_ascii_case(DEFAULT) || others.iter().any(|other| same(other, name))
    };
    free(wanted, NEW, others.len(), taken)
}

/// `wanted` trimmed and shortened to [`MAX_CHARS`] (`fallback` when that leaves nothing), or
/// with the first free number after it when `taken` says it is. `count` is how many names
/// there are, so a free number comes before `count + 2`.
pub fn free(wanted: &str, fallback: &str, count: usize, taken: impl Fn(&str) -> bool) -> String {
    let wanted: String = wanted.trim().chars().take(MAX_CHARS).collect();
    let wanted = if wanted.is_empty() { fallback.to_owned() } else { wanted };
    if !taken(&wanted) {
        return wanted;
    }
    let base = without_number(&wanted);
    // Fewer names than numbers, so a free one comes before the range ends.
    (2..=count + 2).map(|n| format!("{base} ({n})")).find(|name| !taken(name)).unwrap_or(wanted)
}

/// Whether names `a` and `b` are the same, ignoring case.
pub fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// `name` without a trailing " (2)", so copying "Anime (2)" makes "Anime (3)".
fn without_number(name: &str) -> &str {
    let Some(open) = name.strip_suffix(')').and_then(|rest| rest.rfind(" (")) else {
        return name;
    };
    let digits = &name[open + 2..name.len() - 1];
    if !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()) {
        &name[..open]
    } else {
        name
    }
}
