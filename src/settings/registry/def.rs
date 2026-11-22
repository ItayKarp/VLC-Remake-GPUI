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
    /// A slider over the setting's [`NumberRange`]; `format` labels a value, and `marks` puts
    /// ticks on the track.
    Slider { marks: Marks, format: fn(f64) -> String },
    /// A number field showing `unit` after the value ("10 s").
    Number { unit: &'static str },
    /// A text field.
    Text,
    /// A path field with a Browse button that picks as `PathPick` says.
    Path(PathPick),
    /// A color field (`#RRGGBB`, `#RGB` or a name) with a square that opens a color picker.
    HexColor,
    /// A button that runs `action` ("Run Setup Again…"); its binding is [`Binding::ACTION`].
    Button { label: &'static str, action: fn(&mut gpui::Window, &mut gpui::App) },
    /// A comma-separated list of entries.
    List,
    /// A password kept in the OS keychain, never in the settings; its binding is
    /// [`Binding::ACTION`].
    Secret(Secret),
    /// The equalizer's sliders, across the page under the rows before it; the panel saves the
    /// gains itself, so its binding is [`Binding::ACTION`].
    Equalizer,
    /// The library's media folders (S5.9.3), across the page: each with its status, Rescan and
    /// Remove, then buttons that add one. The list changes the folders itself, so its binding is
    /// [`Binding::ACTION`].
    MediaFolders,
}

/// The ticks on a [`Control::Slider`]'s track, every that much from the minimum.
#[derive(Clone, Copy, Debug)]
pub enum Marks {
    /// A plain track.
    None,
    /// Ticks only.
    Ticks(f64),
    /// Ticks, each labelled under the track by the function (an empty label leaves a tick
    /// unlabelled).
    Labelled(f64, fn(f64) -> String),
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
    /// A folder that exists (where recordings go).
    Folder,
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

/// What the OS's alert asks before a toggle is turned off, because off is risky ("Verify
/// certificates").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Confirm {
    /// The alert's question.
    pub title: &'static str,
    /// What turning it off means.
    pub detail: &'static str,
    /// The button that turns it off; the other one cancels.
    pub button: &'static str,
}

/// A button beside a text field that acts on its saved value (the proxy's Test).
#[derive(Clone, Copy, Debug)]
pub struct SideButton {
    /// The button's label.
    pub label: &'static str,
    /// What it does.
    pub action: fn(&mut gpui::Window, &mut gpui::App),
}

/// Checks a value beyond its range; returns the message shown under the field.
pub type Validator = fn(&Value) -> Result<(), String>;

/// How a row's hint line reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HintTone {
    /// Muted, like the label's other notes.
    #[default]
    Note,
    /// In the danger color: the value puts the user at risk ("Certificates aren't verified").
    Danger,
}

/// A line under the row's label about its value in the light of the other settings or the
/// machine ("Starts at 150%, the maximum volume"); `None` when there is nothing to say.
pub type Hint = fn(&gpui::App) -> Option<String>;

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
    /// A line under the label, given the settings.
    pub hint: Option<Hint>,
    /// How the hint line reads.
    pub hint_tone: HintTone,
    /// What an empty text field shows.
    pub placeholder: &'static str,
    /// What a toggle asks before it is turned off.
    pub confirm_off: Option<Confirm>,
    /// A button beside the row's text field.
    pub side_button: Option<SideButton>,
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
            hint: None,
            hint_tone: HintTone::Note,
            placeholder: "",
            confirm_off: None,
            side_button: None,
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

    /// Shows `hint`'s line under the label.
    pub const fn hint(mut self, hint: Hint) -> Self {
        self.hint = Some(hint);
        self
    }

    /// Shows `hint`'s line under the label in the danger color.
    pub const fn danger_hint(mut self, hint: Hint) -> Self {
        self.hint = Some(hint);
        self.hint_tone = HintTone::Danger;
        self
    }

    /// Shows `placeholder` in the row's empty text field.
    pub const fn placeholder(mut self, placeholder: &'static str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Asks `confirm` before the toggle is turned off.
    pub const fn confirm_off(mut self, confirm: Confirm) -> Self {
        self.confirm_off = Some(confirm);
        self
    }

    /// Puts a button labelled `label` that runs `action` beside the row's text field.
    pub const fn side_button(
        mut self,
        label: &'static str,
        action: fn(&mut gpui::Window, &mut gpui::App),
    ) -> Self {
        self.side_button = Some(SideButton { label, action });
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
