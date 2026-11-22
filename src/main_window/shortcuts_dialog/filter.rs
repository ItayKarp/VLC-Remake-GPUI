//! The Keyboard Shortcuts dialog's rows and its search (S2.10.4), kept apart from drawing so
//! they can be tested.

use gpui::{OperatingSystem, SharedString};

use crate::keymap::custom::Shortcuts;
use crate::keymap::sections::{SECTIONS, Section};
use crate::keymap::{commands, label, menus};

/// Where the search looks: the dropdown beside the search field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    /// The command's name and its keys.
    All,
    /// The command's name.
    Action,
    /// The command's keys.
    Hotkey,
}

impl Field {
    /// The choices, in the dropdown's order.
    pub const ALL: [Field; 3] = [Field::All, Field::Action, Field::Hotkey];

    /// The dropdown's text for it.
    pub fn label(self) -> &'static str {
        match self {
            Field::All => "All Fields",
            Field::Action => "Action",
            Field::Hotkey => "Hotkey",
        }
    }

    /// The choice at `index` in [`Self::ALL`]; all fields past the end.
    pub fn at(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Field::All)
    }
}

/// One command's row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The command's index in the command table.
    pub index: usize,
    /// The command's name.
    pub label: &'static str,
    /// Its shortcuts as text, the primary one first.
    pub keys: Vec<SharedString>,
    /// Whether its keys differ from the defaults.
    pub changed: bool,
}

impl Row {
    /// Whether the row answers `query` in `field`: every word of it appears, ignoring case.
    /// Keys match however they're spaced ("ctrl+o" finds "Ctrl + O").
    pub fn matches(&self, query: &str, field: Field) -> bool {
        let label = self.label.to_lowercase();
        let keys = compact(&self.keys.join(" "));
        query.split_whitespace().all(|word| {
            let in_label = field != Field::Hotkey && label.contains(&word.to_lowercase());
            let in_keys = field != Field::Action && keys.contains(&compact(word));
            in_label || in_keys
        })
    }
}

/// One section's heading and rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The heading.
    pub title: &'static str,
    /// The rows, in order.
    pub rows: Vec<Row>,
}

/// Every section with its rows as `shortcuts` has them, leaving out commands whose stories
/// haven't shipped (S2.9.4).
pub fn groups(shortcuts: &Shortcuts) -> Vec<Group> {
    let mac = shortcuts.os() == OperatingSystem::Mac;
    let row = |action| {
        let index = Shortcuts::index_of(action)?;
        let command = commands::find(action)?;
        let keys = shortcuts.keys(index).iter().map(|key| label::text(key, mac).into()).collect();
        let changed = !shortcuts.is_default(index);
        (!menus::is_pending(action)).then_some(Row { index, label: command.label, keys, changed })
    };
    let group = |section: &Section| Group {
        title: section.title,
        rows: section.actions.iter().filter_map(|action| row(*action)).collect(),
    };
    SECTIONS.iter().map(group).collect()
}

/// The groups that answer `query` in `field`, with only their matching rows; all of them for
/// an empty query.
pub fn search(groups: &[Group], query: &str, field: Field) -> Vec<Group> {
    groups
        .iter()
        .map(|group| Group {
            title: group.title,
            rows: group.rows.iter().filter(|row| row.matches(query, field)).cloned().collect(),
        })
        .filter(|group| !group.rows.is_empty())
        .collect()
}

/// `text` lowercased without spaces or `+`, so key names compare however they're written.
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace() && *c != '+').flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(label: &'static str, keys: &[&'static str]) -> Row {
        let keys = keys.iter().map(|key| SharedString::from(*key)).collect();
        Row { index: 0, label, keys, changed: false }
    }

    fn sample() -> Vec<Group> {
        vec![
            Group { title: "Basic", rows: vec![row("Toggle fullscreen", &["F"])] },
            Group {
                title: "Open",
                rows: vec![row("Open File(s)", &["Ctrl + O"]), row("Open Folder", &[])],
            },
        ]
    }

    #[test]
    fn an_empty_search_keeps_everything() {
        assert_eq!(search(&sample(), "  ", Field::All), sample());
    }

    #[test]
    fn searching_names_drops_empty_sections() {
        let found = search(&sample(), "OPEN fol", Field::Action);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rows, vec![row("Open Folder", &[])]);
    }

    #[test]
    fn keys_match_however_they_are_spaced() {
        let found = search(&sample(), "ctrl+o", Field::Hotkey);
        assert_eq!(found[0].rows, vec![row("Open File(s)", &["Ctrl + O"])]);
    }

    #[test]
    fn each_field_looks_only_where_it_says() {
        assert!(!row("Toggle fullscreen", &["F"]).matches("toggle", Field::Hotkey));
        assert!(!row("Open File(s)", &["Ctrl + O"]).matches("ctrl", Field::Action));
        assert!(row("Open File(s)", &["Ctrl + O"]).matches("file ctrl", Field::All));
    }

    #[test]
    fn a_choice_past_the_end_means_all_fields() {
        assert_eq!(Field::at(2), Field::Hotkey);
        assert_eq!(Field::at(9), Field::All);
    }
}
