//! Dropdown / select (S0.4.3): `shared/components/dropdown-*`,
//! `settings/components/setting_preset-*`.
//!
//! Closed: a well-framed control face with the value and a chevron. Open: a list of
//! option rows under it (above when there's no room), the chosen one with an
//! accent ✓, scrolling when there are many.
//!
//! Keyboard: Enter, Space, Alt+Down or Down open it; Up/Down/Home/End/PageUp/
//! PageDown move the highlight; Enter or Space chooses; Escape or Tab closes.
//! Typing selects by prefix (closed) or highlights (open); repeating one letter
//! cycles through the items that start with it.

mod keys;
mod render;
mod type_ahead;

use gpui::{
    App, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, ScrollHandle, SharedString,
    Window,
};

pub use type_ahead::{TypeAhead, find_match};

use super::popover::{self, BoundsTracker};
use crate::theme;

/// Rows shown before the list scrolls.
const VISIBLE_ROWS: usize = 8;

/// The value changed (by mouse, keyboard or typing).
pub struct Changed(pub usize);

/// A select control: shows the chosen option and opens a list to pick another.
/// Emits [`Changed`] when the user picks, and [`DismissEvent`] when the list closes.
pub struct Dropdown {
    options: Vec<SharedString>,
    selected: usize,
    label: SharedString,
    open: bool,
    highlighted: Option<usize>,
    focus_handle: FocusHandle,
    trigger: BoundsTracker,
    list: BoundsTracker,
    scroll: ScrollHandle,
    type_ahead: TypeAhead,
    disabled: bool,
}

impl EventEmitter<Changed> for Dropdown {}
impl EventEmitter<DismissEvent> for Dropdown {}

impl Focusable for Dropdown {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Dropdown {
    /// Creates a closed dropdown showing `options[selected]`. `label` names the
    /// setting for screen readers.
    pub fn new(
        label: impl Into<SharedString>,
        options: Vec<SharedString>,
        selected: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        theme::observe(window, cx);
        popover::close_on_deactivate(window, cx, |this: &mut Self, _, cx| this.close(cx));
        let focus_handle = cx.focus_handle().tab_stop(true);
        // Tabbing away closes the list.
        cx.on_blur(&focus_handle, window, |this, _, cx| this.close(cx)).detach();
        Self {
            selected: selected.min(options.len().saturating_sub(1)),
            options,
            label: label.into(),
            open: false,
            highlighted: None,
            focus_handle,
            trigger: BoundsTracker::new(),
            list: BoundsTracker::new(),
            scroll: ScrollHandle::new(),
            type_ahead: TypeAhead::default(),
            disabled: false,
        }
    }

    /// The index of the chosen option.
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Chooses an option from outside (settings). Doesn't emit.
    #[allow(dead_code, reason = "kit API for later stages")]
    pub fn set_selected(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.options.len() && index != self.selected {
            self.selected = index;
            cx.notify();
        }
    }

    /// Replaces the options, keeping the selection in range.
    #[allow(dead_code, reason = "kit API for later stages")]
    pub fn set_options(&mut self, options: Vec<SharedString>, cx: &mut Context<Self>) {
        self.selected = self.selected.min(options.len().saturating_sub(1));
        self.options = options;
        self.highlighted = None;
        cx.notify();
    }

    /// Greys the dropdown out and takes it out of the tab order (closing it).
    #[allow(dead_code, reason = "kit API for later stages")]
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        if disabled {
            self.close(cx);
        }
        cx.notify();
    }

    /// Picks `index`, closing the list and emitting [`Changed`] if it moved.
    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.options.len() {
            return;
        }
        let changed = index != self.selected;
        self.selected = index;
        self.close(cx);
        if changed {
            cx.emit(Changed(index));
        }
    }

    /// Selects `index` while closed, like a native select, emitting [`Changed`].
    fn select_closed(&mut self, index: usize, cx: &mut Context<Self>) {
        if index != self.selected {
            self.selected = index;
            cx.emit(Changed(index));
            cx.notify();
        }
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.open {
            self.close(cx);
        } else {
            self.open(window, cx);
        }
    }

    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.options.is_empty() {
            return;
        }
        self.focus_handle.focus(window, cx);
        self.open = true;
        self.highlighted = Some(self.selected);
        self.scroll.scroll_to_item(self.selected);
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.open {
            self.open = false;
            self.highlighted = None;
            cx.emit(DismissEvent);
            cx.notify();
        }
    }
}
