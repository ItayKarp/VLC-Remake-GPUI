//! One registry entry, [`SettingDef`], and the pieces it is made of: the control a row shows,
//! the range a number keeps to, the setting it depends on, and when a change takes effect.

use gpui::Hsla;

use super::category::Category;
use super::value::{Binding, Choice, Value};
use crate::privacy::keychain::Secret;
use crate::settings::Settings;

/// The control a row shows.
#[derive(Clone, Copy, Debug)]
pub enum Control {
    /// An ON/OFF switch.
    Toggle,
    /// A dropdown of choices.
    Dropdown(&'static [Choice]),
    /// A dropdown of what this machine has (its displays): `options` lists them, given the
    /// saved id, which is listed even when it's gone. Its binding holds the id as text.
    Options(OptionsSource),
    /// Two or three segments side by side.
    Segmented(&'static [Choice]),
    /// Color squares, each a choice; `color` gives each one's color by id.
    Swatches { choices: &'static [Choice], color: fn(&str) -> Option<Hsla> },
    /// A slider over the setting's [`NumberRange`]; `format` labels a value. With `marks`, a
    /// tick and a label every that much from the minimum.
    Slider { marks: Option<f64>, format: fn(f64) -> String },
    /// A number field showing `unit` after the value ("10 s").
    Number { unit: &'static str },
    /// A text field.
    Text,
    /// A path field with a Browse button that picks as `PathPick` says.
    Path(PathPick),
    /// A `#RRGGBB` field with a color square.
    #[allow(dead_code, reason = "color rows come with S6 (subtitle colors)")]
    HexColor,
    /// A button that runs `action` ("Run Setup Again…"); its binding is [`Binding::ACTION`].
    Button { label: &'static str, action: fn(&mut gpui::Window, &mut gpui::App) },
    /// A comma-separated list of entries.
    List,
    /// A password kept in the OS keychain, never in the settings; its binding is
    /// [`Binding::ACTION`].
    Secret(Secret),
}

/// Lists an [`Control::Options`] row's options, given the saved id.
pub type OptionsSource = fn(&gpui::App, &str) -> Vec<MachineChoice>;

/// One option of a [`Control::Options`] row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineChoice {
    /// What the setting stores.
    pub id: String,
    /// What the dropdown shows.
    pub label: String,
}

/// What a path row's Browse button asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathPick {
    /// A file that exists (a skin).
    #[cfg_attr(
        not(feature = "skins"),
        allow(dead_code, reason = "the skin file row only shows with the `skins` feature")
    )]
    ExistingFile,
    /// Where to write a file, which may not exist yet (a log); `name` is suggested.
    NewFile { name: &'static str },
}

/// The numbers a numeric setting allows, in the unit its row shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberRange {
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// How finely the value moves; values snap to `min + n * step`.
    pub step: f64,
}

impl NumberRange {
    /// From `min` to `max` in steps of `step`.
    pub const fn new(min: f64, max: f64, step: f64) -> Self {
        Self { min, max, step }
    }
}

/// When a parent setting lets its dependent rows be changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    /// The parent toggle is on.
    IsOn,
    /// The parent has this choice.
    Is(&'static str),
    /// The parent has any choice but this one.
    IsNot(&'static str),
}

impl Condition {
    /// Whether the parent's `value` meets the condition.
    pub fn holds(self, value: &Value) -> bool {
        match (self, value) {
            (Self::IsOn, Value::Bool(on)) => *on,
            (Self::Is(want), Value::Choice(id)) => want == *id,
            (Self::IsNot(unwanted), Value::Choice(id)) => unwanted != *id,
            _ => false,
        }
    }
}

/// A row that is dimmed and disabled unless another setting has some value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// The parent setting's key.
    pub key: &'static str,
    /// What the parent must be.
    pub condition: Condition,
}

/// When a change takes effect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApplyMode {
    /// At once.
    #[default]
    Live,
    /// With the next file opened: the row says so.
    NextMedia,
    /// At the next launch: the page offers to restart.
    Restart,
}

/// Checks a value beyond its range; returns the message shown under the field.
pub type Validator = fn(&Value) -> Result<(), String>;

/// One setting: everything the Settings window needs to draw, search, validate and save it.
#[derive(Clone, Copy, Debug)]
pub struct SettingDef {
    /// A unique, stable id: the field's path in the settings file (`player.resume`).
    pub key: &'static str,
    /// The page the row is on.
    pub category: Category,
    /// The heading the row is under.
    pub section: &'static str,
    /// The row's label.
    pub label: &'static str,
    /// The `(i)` tooltip.
    pub info: &'static str,
    /// The control.
    pub control: Control,
    /// Where the value lives.
    pub binding: Binding,
    /// The range a number keeps to.
    pub range: Option<NumberRange>,
    /// A check beyond the range.
    pub validate: Option<Validator>,
    /// The setting this row depends on.
    pub dependency: Option<Dependency>,
    /// When a change takes effect.
    pub apply: ApplyMode,
    /// Whether the row is under the page's Advanced Options.
    pub advanced: bool,
    /// Other words search finds the row by.
    pub keywords: &'static [&'static str],
}

impl SettingDef {
    /// A live, non-advanced row with no range, check, dependency or keywords.
    pub const fn new(
        key: &'static str,
        place: (Category, &'static str),
        label: &'static str,
        control: Control,
        binding: Binding,
    ) -> Self {
        Self {
            key,
            category: place.0,
            section: place.1,
            label,
            info: "",
            control,
            binding,
            range: None,
            validate: None,
            dependency: None,
            apply: ApplyMode::Live,
            advanced: false,
            keywords: &[],
        }
    }

    /// Sets the `(i)` tooltip.
    pub const fn info(mut self, info: &'static str) -> Self {
        self.info = info;
        self
    }

    /// Keeps the number within `min..=max`, moving in steps of `step`.
    pub const fn range(mut self, min: f64, max: f64, step: f64) -> Self {
        self.range = Some(NumberRange::new(min, max, step));
        self
    }

    /// Adds a check beyond the range.
    pub const fn validate(mut self, validate: Validator) -> Self {
        self.validate = Some(validate);
        self
    }

    /// Enables the row only while the setting `key` meets `condition`.
    pub const fn depends_on(mut self, key: &'static str, condition: Condition) -> Self {
        self.dependency = Some(Dependency { key, condition });
        self
    }

    /// Sets when a change takes effect.
    pub const fn applies(mut self, apply: ApplyMode) -> Self {
        self.apply = apply;
        self
    }

    /// Puts the row under Advanced Options.
    pub const fn advanced(mut self) -> Self {
        self.advanced = true;
        self
    }

    /// Adds words search finds the row by.
    pub const fn keywords(mut self, keywords: &'static [&'static str]) -> Self {
        self.keywords = keywords;
        self
    }

    /// The setting's current value.
    pub fn value(&self, settings: &Settings) -> Value {
        (self.binding.get)(settings)
    }

    /// The setting's default value.
    pub fn default_value(&self) -> Value {
        self.value(&Settings::default())
    }

    /// The choices of a dropdown, segmented control or swatch row.
    pub fn choices(&self) -> &'static [Choice] {
        match self.control {
            Control::Dropdown(choices)
            | Control::Segmented(choices)
            | Control::Swatches { choices, .. } => choices,
            _ => &[],
        }
    }
}
