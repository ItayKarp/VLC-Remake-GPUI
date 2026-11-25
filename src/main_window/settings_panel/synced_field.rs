//! A typed field that shows a value in effect, such as Rotate's angle or a crop edge.
//!
//! The panel syncs its fields as it draws. Leaving a field (a click or Tab elsewhere) redraws
//! the panel before the field hears it lost the focus, so a sync that put the value in effect
//! back into every field not focused would replace the typed text before it is taken. A field
//! is therefore rewritten only when the value in effect changes, as the speed field and the
//! number rows do; leaving it takes the typed text, or puts the value back, itself.

use gpui::{App, Entity, Window};

use crate::ui::text_input::{TextInput, Validation};

/// A text field and the value in effect it last showed.
pub(super) struct SyncedField {
    field: Entity<TextInput>,
    /// The value in effect, as text, when it last changed; `None` before the first sync.
    shown: Option<String>,
}

impl SyncedField {
    /// Wraps `field`; the first sync shows the value in effect in it.
    pub(super) fn new(field: Entity<TextInput>) -> Self {
        Self { field, shown: None }
    }

    /// The field.
    pub(super) fn field(&self) -> &Entity<TextInput> {
        &self.field
    }

    /// Shows `text`, the value in effect, without an error, when it changed since the last
    /// sync. A field being typed in keeps its text, as does one whose value didn't change.
    pub(super) fn sync(&mut self, text: String, window: &Window, cx: &mut App) {
        if self.shown.as_ref() == Some(&text) {
            return;
        }
        self.shown = Some(text.clone());
        if self.field.read(cx).is_focused(window) {
            return;
        }
        self.field.update(cx, |field, cx| {
            field.set_text(text, cx);
            field.set_validation(Validation::None, cx);
        });
    }
}
