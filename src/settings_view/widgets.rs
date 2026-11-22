//! The page's stateful controls: a dropdown, slider or text field per row that needs one, made
//! once per page and kept in step with the settings.
//!
//! Text fields (numbers, lists, paths, colors) take any text while they're edited, so "-", "" and
//! "1." are fine mid-typing. Enter or leaving the field checks it: a good value is saved and shown
//! tidied ("10" becomes "10 s"), a bad one shows why under the field on Enter and is dropped when
//! the field loses focus. Escape puts the saved value back. A half-typed value is never saved.

use std::collections::HashMap;

use gpui::{
    AnyElement, Context, Entity, KeyDownEvent, SharedString, Subscription, Window, div, prelude::*,
};

use super::SettingsPage;
use super::options::OptionsDropdown;
use super::secret::SecretField;
use crate::settings::{
    self, Settings,
    registry::{Control, SettingDef, Value},
};
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::slider::{self, Slider, SliderModel};
use crate::ui::text_input::{self, TextInput, Validation};

/// One row's stateful control.
enum Widget {
    Dropdown(Entity<Dropdown>),
    Options(OptionsDropdown),
    Slider(Entity<Slider>),
    Field(Entity<TextInput>),
    Secret(Entity<SecretField>),
}

/// A control and the subscription to what the user does with it.
type Made = (Widget, Subscription);

/// The stateful controls of a page's rows, by setting key.
pub(super) struct Widgets {
    by_key: HashMap<&'static str, Widget>,
    subscriptions: Vec<Subscription>,
}

impl Widgets {
    /// The controls of `defs` that need one.
    pub(super) fn new(
        defs: &[&'static SettingDef],
        window: &mut Window,
        cx: &mut Context<SettingsPage>,
    ) -> Self {
        let mut widgets = Self { by_key: HashMap::new(), subscriptions: Vec::new() };
        for def in defs {
            let made = match def.control {
                Control::Dropdown(_) => Some(make_dropdown(def, window, cx)),
                Control::Options(source) => {
                    let (options, subscription) = OptionsDropdown::new(def, source, window, cx);
                    Some((Widget::Options(options), subscription))
                }
                Control::Slider { .. } => make_slider(def, window, cx),
                Control::Number { .. }
                | Control::Text
                | Control::Path(_)
                | Control::HexColor
                | Control::List => Some(make_field(def, window, cx)),
                Control::Secret(secret) => {
                    let field = cx.new(|cx| SecretField::new(secret, def.label, window, cx));
                    // The field saves to the keychain itself; the page only keeps it enabled.
                    widgets.by_key.insert(def.key, Widget::Secret(field));
                    None
                }
                _ => None,
            };
            if let Some((widget, subscription)) = made {
                widgets.by_key.insert(def.key, widget);
                widgets.subscriptions.push(subscription);
            }
        }
        widgets
    }

    /// The row's control as an element, if it has a stateful one.
    pub(super) fn element(&self, def: &SettingDef) -> Option<AnyElement> {
        Some(match self.by_key.get(def.key)? {
            Widget::Dropdown(dropdown) => dropdown.clone().into_any_element(),
            Widget::Options(options) => options.dropdown.clone().into_any_element(),
            Widget::Slider(slider) => slider.clone().into_any_element(),
            Widget::Field(field) => field.clone().into_any_element(),
            Widget::Secret(field) => field.clone().into_any_element(),
        })
    }

    /// The row's text field, if it has one.
    pub(super) fn field(&self, key: &str) -> Option<&Entity<TextInput>> {
        match self.by_key.get(key)? {
            Widget::Field(field) => Some(field),
            _ => None,
        }
    }

    /// Shows each control's saved value and whether its row is enabled. A field being edited
    /// keeps what is typed.
    pub(super) fn sync(
        &self,
        defs: &[&'static SettingDef],
        window: &mut Window,
        cx: &mut Context<SettingsPage>,
    ) {
        let settings = settings::get(cx).clone();
        for def in defs {
            if let Some(widget) = self.by_key.get(def.key) {
                sync_widget(widget, def, &settings, window, cx);
            }
        }
    }
}

fn sync_widget(
    widget: &Widget,
    def: &SettingDef,
    settings: &Settings,
    window: &mut Window,
    cx: &mut Context<SettingsPage>,
) {
    let value = def.value(settings);
    let disabled = !def.is_enabled(settings);
    match widget {
        Widget::Dropdown(dropdown) => dropdown.update(cx, |dropdown, cx| {
            dropdown.set_selected(choice_index(def, &value), cx);
            dropdown.set_disabled(disabled, cx);
        }),
        Widget::Options(options) => {
            if let Control::Options(source) = def.control {
                options.sync(def, source, settings, cx);
            }
        }
        Widget::Slider(slider) => slider.update(cx, |slider, cx| {
            let Value::Number(number) = value else { return };
            slider.set_value(number, window, cx);
            slider.set_disabled(disabled, cx);
        }),
        Widget::Secret(field) => field.update(cx, |field, cx| field.set_parent_off(disabled, cx)),
        Widget::Field(field) => field.update(cx, |field, cx| {
            field.set_disabled(disabled, cx);
            let text = def.format_value(&value);
            if !field.is_focused(window) && !field.is_invalid() && field.text() != text {
                field.set_text(text, cx);
            }
        }),
    }
}

fn make_dropdown(
    def: &'static SettingDef,
    window: &mut Window,
    cx: &mut Context<SettingsPage>,
) -> Made {
    let labels = def.choices().iter().map(|choice| SharedString::from(choice.label)).collect();
    let selected = choice_index(def, &def.value(settings::get(cx)));
    let dropdown = cx.new(|cx| Dropdown::new(def.label, labels, selected, window, cx));
    let subscription = cx.subscribe(&dropdown, move |page, _, event: &dropdown::Changed, cx| {
        page.write_choice(def, event.0, cx);
    });
    (Widget::Dropdown(dropdown), subscription)
}

fn make_slider(
    def: &'static SettingDef,
    window: &mut Window,
    cx: &mut Context<SettingsPage>,
) -> Option<Made> {
    let (Control::Slider { marks, format }, Some(range)) = (def.control, def.range) else {
        return None;
    };
    let Value::Number(default) = def.default_value() else { return None };
    let slider = cx.new(|cx| {
        let model = SliderModel::new(range.min, range.max, range.step, default);
        let slider = Slider::new(def.label, model, window, cx).accent().format(format);
        match marks {
            Some(every) => slider.marks(every, format),
            None => slider,
        }
    });
    let subscription = cx.subscribe(&slider, move |page, _, event: &slider::Changed, cx| {
        page.write(def, Value::Number(event.0), cx);
    });
    Some((Widget::Slider(slider), subscription))
}

fn make_field(
    def: &'static SettingDef,
    window: &mut Window,
    cx: &mut Context<SettingsPage>,
) -> Made {
    let text = def.format_value(&def.value(settings::get(cx)));
    let field = cx.new(|cx| TextInput::new(window, cx).with_text(text).with_label(def.label));
    let subscription =
        cx.subscribe(&field, move |page, field, event: &text_input::Event, cx| match event {
            text_input::Event::Changed => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            text_input::Event::Submitted => {
                page.commit_field(def, &field, cx);
            }
            text_input::Event::Blurred => {
                if !page.commit_field(def, &field, cx) {
                    show_saved(def, &field, cx);
                }
            }
        });
    (Widget::Field(field), subscription)
}

impl SettingsPage {
    /// Saves what the field holds if it is a good value, showing it tidied, or shows why not.
    /// Returns whether it was saved.
    fn commit_field(
        &mut self,
        def: &'static SettingDef,
        field: &Entity<TextInput>,
        cx: &mut Context<Self>,
    ) -> bool {
        let typed = def.parse_text(field.read(cx).text()).and_then(|value| def.accept(value));
        match typed {
            Ok(value) => {
                self.write(def, value, cx);
                show_saved(def, field, cx);
                true
            }
            Err(invalid) => {
                let message = Validation::Error(invalid.0.into());
                field.update(cx, |field, cx| field.set_validation(message, cx));
                false
            }
        }
    }

    /// Puts `text` in the row's field and saves it as if it were typed and confirmed (a path
    /// from Browse), showing why under the field when it is refused.
    pub(super) fn commit_text(
        &mut self,
        def: &'static SettingDef,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let Some(field) = self.widgets.field(def.key).cloned() else { return };
        field.update(cx, |field, cx| field.set_text(text, cx));
        self.commit_field(def, &field, cx);
    }

    /// The field wrapped so Escape puts the saved value back before anything else handles it.
    pub(super) fn field_with_escape(
        def: &'static SettingDef,
        element: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w_full()
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key != "escape" || event.keystroke.modifiers.modified() {
                    return;
                }
                if let Some(field) = this.widgets.field(def.key).cloned() {
                    show_saved(def, &field, cx);
                    cx.stop_propagation();
                }
            }))
            .child(element)
            .into_any_element()
    }
}

/// Shows the saved value in the field, without an error.
fn show_saved(def: &SettingDef, field: &Entity<TextInput>, cx: &mut Context<SettingsPage>) {
    let text = def.format_value(&def.value(settings::get(cx)));
    field.update(cx, |field, cx| {
        if field.text() != text {
            field.set_text(text, cx);
        }
        field.set_validation(Validation::None, cx);
    });
}

/// The position of `value`'s choice among the row's choices.
fn choice_index(def: &SettingDef, value: &Value) -> usize {
    let Value::Choice(id) = value else { return 0 };
    def.choices().iter().position(|choice| choice.id == *id).unwrap_or_default()
}
