//! What a registry row reads and writes: a [`Value`] moved in and out of the typed [`Settings`]
//! through a [`Binding`], and the [`Choice`]s of dropdowns, segmented controls and swatches.
//!
//! Bindings are plain function pointers so the registry can be a `static` table. The macros
//! [`toggle`], [`number`] and [`choice`] write the common ones for a field path.

use crate::settings::Settings;

/// A setting's value as rows show and edit it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A toggle.
    Bool(bool),
    /// One of a control's [`Choice`]s, by id.
    Choice(&'static str),
    /// A number, in the unit the row shows.
    Number(f64),
    /// Free text: a path, a color, a pattern.
    Text(String),
    /// Several entries, most important first (preferred languages).
    List(Vec<String>),
    /// Several numbers, in the unit the control shows (the equalizer's gains).
    Numbers(Vec<f64>),
}

/// One option of a dropdown, segmented control or swatch row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Choice {
    /// The stable id rows and dependencies name it by.
    pub id: &'static str,
    /// What the control shows (a swatch's accessible name).
    pub label: &'static str,
}

impl Choice {
    /// An option with `id`, shown as `label`.
    pub const fn new(id: &'static str, label: &'static str) -> Self {
        Self { id, label }
    }
}

/// An enum setting a row picks with a dropdown, segmented control or swatches. `VALUES` and
/// `CHOICES` are paired by position.
pub trait SettingChoice: Copy + PartialEq + 'static {
    /// Every value, in the order the control lists them.
    const VALUES: &'static [Self];
    /// Each value's id and label, in the same order.
    const CHOICES: &'static [Choice];

    /// The value's choice id.
    fn choice_id(self) -> &'static str {
        let index = Self::VALUES.iter().position(|value| *value == self).unwrap_or_default();
        Self::CHOICES.get(index).map_or("", |choice| choice.id)
    }

    /// The value whose choice id is `id`.
    fn from_choice_id(id: &str) -> Option<Self> {
        let index = Self::CHOICES.iter().position(|choice| choice.id == id)?;
        Self::VALUES.get(index).copied()
    }
}

/// Reads a setting's value.
pub type Getter = fn(&Settings) -> Value;
/// Writes a setting's value; a value of the wrong kind is ignored.
pub type Setter = fn(&mut Settings, Value);

/// Where a row's value lives in [`Settings`].
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    /// Reads it.
    pub get: Getter,
    /// Writes it.
    pub set: Setter,
}

impl Binding {
    /// A button row's binding: it holds no value.
    pub const ACTION: Self = Self { get: |_| Value::Bool(false), set: |_, _| {} };
}

/// A [`Binding`] for a `String` field: `text!(local.log_file)`.
macro_rules! text {
    ($($field:ident).+) => {
        $crate::settings::registry::Binding {
            get: |s| $crate::settings::registry::Value::Text(s.$($field).+.clone()),
            set: |s, value| {
                if let $crate::settings::registry::Value::Text(text) = value {
                    s.$($field).+ = text;
                }
            },
        }
    };
}

/// A [`Binding`] for a `bool` field: `toggle!(prefs.player.stop_after_each_item)`.
macro_rules! toggle {
    ($($field:ident).+) => {
        $crate::settings::registry::Binding {
            get: |s| $crate::settings::registry::Value::Bool(s.$($field).+),
            set: |s, value| {
                if let $crate::settings::registry::Value::Bool(on) = value {
                    s.$($field).+ = on;
                }
            },
        }
    };
}

/// A [`Binding`] for a numeric field of type `$ty`, rounded when it's an integer:
/// `number!(u32: prefs.audio.volume_step_percent)`.
macro_rules! number {
    ($ty:ty : $($field:ident).+) => {
        $crate::settings::registry::Binding {
            get: |s| {
                $crate::settings::registry::Value::Number(
                    $crate::settings::registry::value::widen(s.$($field).+),
                )
            },
            set: |s, value| {
                if let $crate::settings::registry::Value::Number(number) = value {
                    s.$($field).+ = $crate::settings::registry::value::round::<$ty>(number);
                }
            },
        }
    };
}

/// A [`Binding`] for a field whose type implements [`SettingChoice`]:
/// `choice!(prefs.player.resume)`.
macro_rules! choice {
    ($($field:ident).+) => {
        $crate::settings::registry::Binding {
            get: |s| {
                use $crate::settings::registry::SettingChoice as _;
                $crate::settings::registry::Value::Choice(s.$($field).+.choice_id())
            },
            set: |s, value| {
                use $crate::settings::registry::SettingChoice;
                if let $crate::settings::registry::Value::Choice(id) = value
                    && let Some(chosen) = SettingChoice::from_choice_id(id)
                {
                    s.$($field).+ = chosen;
                }
            },
        }
    };
}

pub(crate) use {choice, number, text, toggle};

/// Converts a row's number to a field's numeric type.
pub(crate) trait FromNumber {
    /// `number` as this type: rounded and saturated for integers.
    fn from_number(number: f64) -> Self;
}

impl FromNumber for u32 {
    fn from_number(number: f64) -> Self {
        number.round() as u32
    }
}

impl FromNumber for i32 {
    fn from_number(number: f64) -> Self {
        number.round() as i32
    }
}

impl FromNumber for f32 {
    fn from_number(number: f64) -> Self {
        number as f32
    }
}

/// A stored number as the registry's `f64`, for the [`number`] macro: an `f32` by its shortest
/// decimal, so 0.2 reads as 0.2 rather than 0.200000003, which a step of 0.01 would move.
pub(crate) fn widen<T: Into<f64> + std::fmt::Display>(number: T) -> f64 {
    let exact = number.to_string().parse().ok();
    exact.unwrap_or_else(|| number.into())
}

/// `number` as `T`, for the [`number`] macro.
pub(crate) fn round<T: FromNumber>(number: f64) -> T {
    T::from_number(number)
}
