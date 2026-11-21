//! Single-line text input (S0.4.9), built on GPUI's input example: caret,
//! selection (mouse, Shift+arrows, double-click for a word, select all),
//! clipboard, IME composition, undo/redo, a placeholder and validation states.
//!
//! The editing rules live in [`EditState`], which has no GPUI types, so they're
//! unit tested. Offsets are UTF-8 byte offsets on grapheme boundaries; IME calls
//! arrive in UTF-16 and are converted at the edge.
//!
//! Keys are bound in the `TextInput` context, so single-letter and Space
//! shortcuts elsewhere don't fire while typing (S0.5.2 builds on this).

mod boundaries;
mod edit_state;
mod element;
mod handlers;
mod history;
mod input_handler;
pub mod keys;
mod render;
mod utf16;

use gpui::{
    App, Bounds, Context, EventEmitter, FocusHandle, Focusable, Pixels, ShapedLine, SharedString,
    Window, px,
};

pub use edit_state::EditState;
pub use keys::bind_keys;

use crate::theme;

/// Validation feedback: an error turns the frame red and shows the message.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Validation {
    /// The value is fine; nothing is shown.
    #[default]
    None,
    /// The value is rejected for the given reason, shown under the field.
    Error(SharedString),
}

/// How the field is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Look {
    /// A 32 px well with a border, a focus ring and the validation message under it.
    #[default]
    Framed,
    /// The text alone, in the surrounding text style: the host draws any frame. An error
    /// turns the text red; its message isn't shown.
    Bare,
}

/// What a text input tells its listeners.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The text changed.
    Changed,
    /// Enter was pressed.
    Submitted,
    /// Focus left the input.
    Blurred,
}

/// A single-line text field view.
pub struct TextInput {
    state: EditState,
    placeholder: SharedString,
    label: SharedString,
    validation: Validation,
    look: Look,
    disabled: bool,
    focus_handle: FocusHandle,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// Horizontal scroll, so the caret stays visible in long text.
    scroll_x: Pixels,
    /// A mouse drag is extending the selection.
    selecting: bool,
}

impl EventEmitter<Event> for TextInput {}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextInput {
    /// Creates an empty, enabled input that follows the app theme.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        let focus_handle = cx.focus_handle().tab_stop(true);
        cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.selecting = false;
            cx.emit(Event::Blurred);
            cx.notify();
        })
        .detach();
        cx.on_focus(&focus_handle, window, |_, _, cx| cx.notify()).detach();
        Self {
            state: EditState::new(""),
            placeholder: SharedString::default(),
            label: SharedString::default(),
            validation: Validation::None,
            look: Look::Framed,
            disabled: false,
            focus_handle,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.),
            selecting: false,
        }
    }

    /// Starts the input with `text`, without an undo step.
    pub fn with_text(mut self, text: impl AsRef<str>) -> Self {
        self.state.set_text(text.as_ref());
        self
    }

    /// Starts with the whole text selected, so typing replaces it.
    pub fn with_all_selected(mut self) -> Self {
        self.state.select_all();
        self
    }

    /// Sets how the field is drawn.
    pub fn with_look(mut self, look: Look) -> Self {
        self.look = look;
        self
    }

    /// Sets the faint hint shown while the input is empty.
    pub fn with_placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Names the field for screen readers.
    pub fn with_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Limits the text to `max` characters.
    #[allow(dead_code, reason = "Kit API for later stages.")]
    pub fn with_max_len(mut self, max: usize) -> Self {
        self.state.set_max_len(max);
        self
    }

    /// The current text.
    pub fn text(&self) -> &str {
        self.state.text()
    }

    /// Replaces the text from outside (not as a user edit); clears the undo history.
    pub fn set_text(&mut self, text: impl AsRef<str>, cx: &mut Context<Self>) {
        self.state.set_text(text.as_ref());
        cx.notify();
    }

    /// Shows or clears a validation error.
    pub fn set_validation(&mut self, validation: Validation, cx: &mut Context<Self>) {
        if self.validation != validation {
            self.validation = validation;
            cx.notify();
        }
    }

    /// Enables or disables the input; a disabled input takes no focus, keys or mouse.
    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.disabled = disabled;
        self.focus_handle = self.focus_handle.clone().tab_stop(!disabled);
        cx.notify();
    }

    /// Whether a validation error is showing.
    pub fn is_invalid(&self) -> bool {
        matches!(self.validation, Validation::Error(_))
    }

    /// Whether the input has keyboard focus in `window`.
    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    /// Repaints after an edit and, when it `changed` the text, tells the platform and listeners.
    fn edited(&mut self, changed: bool, window: &mut Window, cx: &mut Context<Self>) {
        if changed {
            window.notify_text_input_changed(&self.focus_handle);
            cx.emit(Event::Changed);
        }
        cx.notify();
    }

    /// Repaints after the caret or selection moved, and tells the platform.
    fn moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.notify_text_selection_changed(&self.focus_handle);
        cx.notify();
    }
}
