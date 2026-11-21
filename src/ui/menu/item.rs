//! The entries of a [`ContextMenu`](super::ContextMenu) and their builders.

use std::rc::Rc;

use gpui::{App, SharedString, Window};

/// What a menu item runs when chosen.
type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

/// One entry in a context menu.
pub enum MenuItem {
    /// A command, with an optional icon and shortcut hint.
    Action {
        /// The text shown.
        label: SharedString,
        /// An icon path from `assets::icons`.
        icon: Option<&'static str>,
        /// The keyboard shortcut shown on the right, as a hint only.
        shortcut: Option<SharedString>,
        /// Greyed out and skipped by the keyboard.
        disabled: bool,
        /// Runs after the menu closes.
        on_select: Activate,
    },
    /// A check item. Radio items are check items too: several in a row with one
    /// checked form a group, and the designs mark both with a ✓.
    Check {
        /// The text shown.
        label: SharedString,
        /// Shows the ✓.
        checked: bool,
        /// Greyed out and skipped by the keyboard.
        disabled: bool,
        /// Runs after the menu closes.
        on_select: Activate,
    },
    /// Opens a nested menu to the side.
    Submenu {
        /// The text shown.
        label: SharedString,
        /// An icon path from `assets::icons`.
        icon: Option<&'static str>,
        /// The nested menu's items; an empty submenu is disabled.
        items: Vec<MenuItem>,
    },
    /// A thin line between groups.
    Separator,
}

impl MenuItem {
    /// A command that runs `on_select` when chosen.
    pub fn action(
        label: impl Into<SharedString>,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        MenuItem::Action {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            on_select: Rc::new(on_select),
        }
    }

    /// A check item; `on_select` is expected to flip whatever `checked` reflects.
    pub fn check(
        label: impl Into<SharedString>,
        checked: bool,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        MenuItem::Check {
            label: label.into(),
            checked,
            disabled: false,
            on_select: Rc::new(on_select),
        }
    }

    /// One choice of a radio group (a check item; see [`MenuItem::Check`]).
    pub fn radio(
        label: impl Into<SharedString>,
        selected: bool,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        MenuItem::Check {
            label: label.into(),
            checked: selected,
            disabled: false,
            on_select: Rc::new(on_select),
        }
    }

    /// An item that opens `items` in a nested menu.
    pub fn submenu(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu { label: label.into(), icon: None, items }
    }

    /// Adds an icon to an action or submenu; other items ignore it.
    pub fn icon(mut self, path: &'static str) -> Self {
        match &mut self {
            MenuItem::Action { icon, .. } | MenuItem::Submenu { icon, .. } => *icon = Some(path),
            _ => {}
        }
        self
    }

    /// Adds a shortcut hint to an action; other items ignore it.
    pub fn shortcut(mut self, hint: impl Into<SharedString>) -> Self {
        if let MenuItem::Action { shortcut, .. } = &mut self {
            *shortcut = Some(hint.into());
        }
        self
    }

    /// Disables an action or check item; other items ignore it.
    pub fn disabled(mut self, value: bool) -> Self {
        match &mut self {
            MenuItem::Action { disabled, .. } | MenuItem::Check { disabled, .. } => {
                *disabled = value;
            }
            _ => {}
        }
        self
    }

    /// The text shown, for type-to-select. Separators have none.
    pub(super) fn label(&self) -> Option<&SharedString> {
        match self {
            MenuItem::Action { label, .. }
            | MenuItem::Check { label, .. }
            | MenuItem::Submenu { label, .. } => Some(label),
            MenuItem::Separator => None,
        }
    }

    /// Separators and disabled items can't be highlighted.
    pub(super) fn selectable(&self) -> bool {
        match self {
            MenuItem::Action { disabled, .. } | MenuItem::Check { disabled, .. } => !disabled,
            MenuItem::Submenu { items, .. } => !items.is_empty(),
            MenuItem::Separator => false,
        }
    }

    /// Whether the item opens a nested menu.
    pub(super) fn is_submenu(&self) -> bool {
        matches!(self, MenuItem::Submenu { .. })
    }
}
