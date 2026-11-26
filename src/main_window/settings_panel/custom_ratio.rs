//! The Video tab's Custom Ratio field (S7.2.3): the shape Aspect Ratio's Custom stretches the
//! picture to, typed as "21:9", "2.35:1" or "2.4". Held for the session like the other Defaults
//! rows (D24). Text that isn't a ratio turns the field red with an example and changes nothing;
//! leaving the field puts the ratio in effect back.

use gpui::{AppContext as _, Context, Entity, Subscription, Window};

use super::synced_field::SyncedField;
use crate::effects;
use crate::main_window::MainWindow;
use crate::settings::registry::Value;
use crate::ui::text_input::{self, TextInput, Validation};
use crate::video::framing::{RATIO_HELP, parse_ratio};

/// The setting the field edits.
const KEY: &str = "video.custom_aspect_ratio";

/// The field, and its subscription.
pub(super) struct CustomRatioField {
    field: SyncedField,
    _events: Subscription,
}

impl CustomRatioField {
    /// The field, changing the session when a ratio is entered.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let field = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(in_effect(cx))
                .with_placeholder("2.4:1")
                .with_label(tr!("Custom Ratio"))
        });
        let events = cx.subscribe(&field, |_, field, event, cx| on_event(&field, event, cx));
        Self { field: SyncedField::new(field), _events: events }
    }

    /// The field to show.
    pub(super) fn field(&self) -> Entity<TextInput> {
        self.field.field().clone()
    }

    /// Shows the ratio in effect unless it is being typed.
    pub(super) fn sync(&mut self, window: &Window, cx: &mut Context<MainWindow>) {
        self.field.sync(in_effect(cx), window, cx);
    }
}

/// Takes a typed ratio, or says what one looks like.
fn on_event(field: &Entity<TextInput>, event: &text_input::Event, cx: &mut Context<MainWindow>) {
    let typed = field.read(cx).text().trim().to_owned();
    let valid = parse_ratio(&typed).is_some();
    match (event, valid) {
        (text_input::Event::Changed, _) => {
            field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
        }
        (text_input::Event::Submitted | text_input::Event::Blurred, true) => {
            effects::set(cx, KEY, Value::Text(typed));
        }
        (text_input::Event::Submitted, false) => field.update(cx, |field, cx| {
            field.set_validation(Validation::Error(crate::i18n::text(RATIO_HELP).into()), cx);
        }),
        (text_input::Event::Blurred, false) => {
            let text = in_effect(cx);
            field.update(cx, |field, cx| {
                field.set_text(text, cx);
                field.set_validation(Validation::None, cx);
            });
        }
    }
}

/// The custom ratio in effect, as typed.
fn in_effect(cx: &gpui::App) -> String {
    effects::effective(cx).prefs.video.custom_aspect_ratio
}
