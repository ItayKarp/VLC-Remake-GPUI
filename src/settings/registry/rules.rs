//! What the registry decides about a setting: whether its row is enabled (following dependency
//! chains), whether a value is acceptable, how a value reads as text in a field and back, and
//! which rows a search finds.

use super::def::{Control, NumberRange, SettingDef};
use super::value::Value;
use super::{all, find};
use crate::library::search::fold;
use crate::settings::Settings;

/// The longest dependency chain followed; longer means a cycle, which a test rules out.
const MAX_CHAIN: usize = 8;
/// The digits of a `#RRGGBB` color.
const HEX_DIGITS: usize = 6;

/// Why a value was refused, as the field shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Invalid(pub String);

impl SettingDef {
    /// Whether the row can be changed: its parent, and the parent's parent, have the values it
    /// needs. A parent that is itself disabled keeps the child disabled.
    pub fn is_enabled(&self, settings: &Settings) -> bool {
        let mut row = self;
        for _ in 0..MAX_CHAIN {
            let Some(dependency) = row.dependency else { return true };
            let Some(parent) = find(dependency.key) else { return false };
            if !dependency.condition.holds(&parent.value(settings)) {
                return false;
            }
            row = parent;
        }
        false
    }

    /// `value` as the setting takes it, snapped to the range's step, or why it can't be.
    pub fn accept(&self, value: Value) -> Result<Value, Invalid> {
        let value = match (value, self.range) {
            (Value::Number(number), _) if !number.is_finite() => {
                return Err(Invalid("Not a number".into()));
            }
            (Value::Number(number), Some(range)) => {
                if number < range.min || number > range.max {
                    return Err(Invalid(out_of_range(range, self.control)));
                }
                Value::Number(snap(number, range))
            }
            (value, _) => value,
        };
        if let (Control::HexColor, Value::Text(text)) = (self.control, &value)
            && hex_color(text).is_none()
        {
            return Err(Invalid("A color such as #FFFFFF".into()));
        }
        if let Some(validate) = self.validate {
            validate(&value).map_err(Invalid)?;
        }
        Ok(value)
    }

    /// `value` pulled into the range, for values read from an older or newer settings file.
    pub fn clamp(&self, value: Value) -> Value {
        match (value, self.range) {
            (Value::Number(number), Some(range)) if number.is_nan() => Value::Number(range.min),
            (Value::Number(number), Some(range)) => {
                Value::Number(number.clamp(range.min, range.max))
            }
            (value, _) => value,
        }
    }

    /// The value as its field shows it: "10 s", "en, fr", a path.
    pub fn format_value(&self, value: &Value) -> String {
        match value {
            Value::Number(number) => match self.control {
                Control::Number { unit } => with_unit(*number, unit),
                _ => trim(*number),
            },
            Value::Text(text) => text.clone(),
            Value::List(items) => items.join(", "),
            Value::Bool(on) => on.to_string(),
            Value::Choice(id) => (*id).to_owned(),
        }
    }

    /// Reads what was typed in the row's field as a value. Half-typed numbers ("-", "1.", "")
    /// are refused, so they are never saved.
    pub fn parse_text(&self, text: &str) -> Result<Value, Invalid> {
        match self.control {
            Control::Number { unit } => parse_number(text, unit).map(Value::Number),
            Control::List => Ok(Value::List(parse_list(text))),
            _ => Ok(Value::Text(text.trim().to_owned())),
        }
    }

    /// Whether every word of `query` (already folded) is in the label, info text or keywords.
    #[allow(dead_code, reason = "the Settings window's search field (S5.2)")]
    pub fn matches(&self, words: &[String]) -> bool {
        let haystack = fold(&format!("{} {} {}", self.label, self.info, self.keywords.join(" ")));
        words.iter().all(|word| haystack.contains(word.as_str()))
    }
}

/// The rows `query` finds, in registry order; accents and case don't matter. An empty query finds
/// nothing.
#[allow(dead_code, reason = "the Settings window's search field (S5.2)")]
pub fn search(query: &str) -> Vec<&'static SettingDef> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_owned).collect();
    if words.is_empty() {
        return Vec::new();
    }
    all().filter(|def| def.matches(&words)).collect()
}

/// Pulls every numeric setting into its range, logging each one moved. Returns whether any was.
pub fn sanitize(settings: &mut Settings) -> bool {
    let mut changed = false;
    for def in all() {
        let value = def.value(settings);
        let clamped = def.clamp(value.clone());
        if clamped != value {
            tracing::warn!("settings: {} was {value:?}, out of range; using {clamped:?}", def.key);
            (def.binding.set)(settings, clamped);
            changed = true;
        }
    }
    changed
}

/// The color `#RRGGBB` names, as `0xRRGGBB`.
pub fn hex_color(text: &str) -> Option<u32> {
    let digits = text.trim().strip_prefix('#')?;
    let valid = digits.len() == HEX_DIGITS && digits.chars().all(|c| c.is_ascii_hexdigit());
    valid.then(|| u32::from_str_radix(digits, 16).ok()).flatten()
}

/// Reads a typed number, with or without `unit` after it; a comma is a decimal point.
fn parse_number(text: &str, unit: &str) -> Result<f64, Invalid> {
    let text = text.trim();
    let text = text.strip_suffix(unit).unwrap_or(text).trim().replace(',', ".");
    let parsed = if text.ends_with('.') { None } else { text.parse::<f64>().ok() };
    parsed.filter(|number| number.is_finite()).ok_or_else(|| Invalid("Not a number".into()))
}

/// Splits a comma-separated list, dropping blanks.
fn parse_list(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|item| !item.is_empty()).map(str::to_owned).collect()
}

/// `number` on the step grid from the range's minimum, kept in range.
fn snap(number: f64, range: NumberRange) -> f64 {
    if range.step <= 0. {
        return number;
    }
    let steps = ((number - range.min) / range.step).round();
    (range.min + steps * range.step).clamp(range.min, range.max)
}

/// The message for a number outside `range`.
fn out_of_range(range: NumberRange, control: Control) -> String {
    let unit = match control {
        Control::Number { unit } => unit,
        _ => "",
    };
    format!("Between {} and {}", with_unit(range.min, unit), with_unit(range.max, unit))
}

/// `number` followed by `unit`: "10 s", but "50%".
fn with_unit(number: f64, unit: &str) -> String {
    match unit {
        "" => trim(number),
        "%" => format!("{}%", trim(number)),
        _ => format!("{} {unit}", trim(number)),
    }
}

/// A number without a trailing `.0`, with at most two decimals.
pub fn trim(number: f64) -> String {
    let text = format!("{number:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}
