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
        /// Why it is disabled, shown on hover.
        reason: Option<SharedString>,
        /// Runs after the menu closes.
        on_select: Activate,
    },
    /// A check item. Radio items are check items too: several in a row with one
    /// checked form a group, and the designs mark both with a ✓.
    Check {
        /// The text shown.
        label: SharedString,
        /// An icon path from `assets::icons`.
        icon: Option<&'static str>,
        /// Shows the ✓.
        checked: bool,
        /// Greyed out and skipped by the keyboard.
        disabled: bool,
        /// Why it is disabled, shown on hover.
        reason: Option<SharedString>,
        /// What it means, shown on hover while it is enabled.
        hint: Option<SharedString>,
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
        /// Why it is disabled (empty), shown on hover.
        reason: Option<SharedString>,
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
            reason: None,
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
            icon: None,
            checked,
            disabled: false,
            reason: None,
            hint: None,
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
            icon: None,
            checked: selected,
            disabled: false,
            reason: None,
            hint: None,
            on_select: Rc::new(on_select),
        }
    }

    /// An item that opens `items` in a nested menu.
    pub fn submenu(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu { label: label.into(), icon: None, items, reason: None }
    }

    /// Adds an icon to an action, a check item or a submenu; a separator ignores it.
    pub fn icon(mut self, path: &'static str) -> Self {
        match &mut self {
            MenuItem::Action { icon, .. }
            | MenuItem::Check { icon, .. }
            | MenuItem::Submenu { icon, .. } => *icon = Some(path),
            MenuItem::Separator => {}
        }
        self
    }

    /// Adds a shortcut hint to an action; other items ignore it.
    pub fn shortcut(mut self, hint: impl Into<SharedString>) -> Self {
        self.set_shortcut(Some(hint.into()));
        self
    }

    /// Adds a shortcut hint to an action when there is one (a command may have no keys).
    pub fn maybe_shortcut(mut self, hint: Option<SharedString>) -> Self {
        self.set_shortcut(hint);
        self
    }

    fn set_shortcut(&mut self, hint: Option<SharedString>) {
        if let MenuItem::Action { shortcut, .. } = self {
            *shortcut = hint;
        }
    }

    /// Says why the item is disabled, on hover ("No media playing"). It shows only while the
    /// item is disabled, or for a submenu, while it is empty.
    pub fn why_disabled(mut self, why: impl Into<SharedString>) -> Self {
        match &mut self {
            MenuItem::Action { reason, .. }
            | MenuItem::Check { reason, .. }
            | MenuItem::Submenu { reason, .. } => *reason = Some(why.into()),
            MenuItem::Separator => {}
        }
        self
    }

    /// Explains what a check item means, on hover ("By the date added to the library"); other
    /// items ignore it.
    pub fn hint(mut self, text: impl Into<SharedString>) -> Self {
        if let MenuItem::Check { hint, .. } = &mut self {
            *hint = Some(text.into());
        }
        self
    }

    /// What hovering the item says: why it is disabled while it is, else its hint.
    pub(super) fn hover_text(&self) -> Option<&SharedString> {
        match self {
            MenuItem::Check { hint: Some(hint), .. } if self.selectable() => Some(hint),
            _ => self.disabled_reason(),
        }
    }

    /// Why the item is disabled, while it is.
    fn disabled_reason(&self) -> Option<&SharedString> {
        match self {
            MenuItem::Action { reason, .. }
            | MenuItem::Check { reason, .. }
            | MenuItem::Submenu { reason, .. }
                if !self.selectable() =>
            {
                reason.as_ref()
            }
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::MenuItem;

    fn reason(item: &MenuItem) -> Option<&str> {
        item.disabled_reason().map(AsRef::as_ref)
    }

    #[test]
    fn the_reason_shows_only_while_disabled() {
        let enabled = MenuItem::action("Stop", |_, _| {}).why_disabled("No media playing");
        assert_eq!(reason(&enabled), None);
        let disabled = enabled.disabled(true);
        assert_eq!(reason(&disabled), Some("No media playing"));
        let check = MenuItem::check("Mute", false, |_, _| {}).disabled(true).why_disabled("No");
        assert_eq!(reason(&check), Some("No"));
    }

    #[test]
    fn a_hint_shows_while_enabled() {
        let check = MenuItem::radio("Newest First", false, |_, _| {}).hint("By date added");
        assert_eq!(check.hover_text().map(AsRef::as_ref), Some("By date added"));
        let disabled = check.disabled(true).why_disabled("No");
        assert_eq!(disabled.hover_text().map(AsRef::as_ref), Some("No"));
    }

    #[test]
    fn an_empty_submenu_says_why() {
        let empty = MenuItem::submenu("Open Recent", Vec::new()).why_disabled("Nothing recent");
        assert_eq!(reason(&empty), Some("Nothing recent"));
        let full = MenuItem::submenu("Open Recent", vec![MenuItem::action("a.mkv", |_, _| {})])
            .why_disabled("Nothing recent");
        assert_eq!(reason(&full), None);
    }
}
