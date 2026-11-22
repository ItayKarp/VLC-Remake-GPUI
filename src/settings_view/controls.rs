//! Each row's control: the stateless ones (toggle, segmented, swatches, button) drawn from the
//! settings on every render, and the stateful widgets with what goes beside them (a path's Browse
//! button, a color's square).

use std::rc::Rc;

use gpui::{AnyElement, App, Context, PathPromptOptions, Window, div, prelude::*, px};

use super::SettingsPage;
use super::swatches::{SelectHandler, SwatchRow, swatches};
use crate::settings::{
    self,
    registry::{self, Control, SettingDef, Value},
};
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::segmented::Segmented;
use crate::ui::toggle::Toggle;

/// The space between a field and its Browse button or color square.
const BESIDE_GAP: f32 = 8.;
/// The square showing a color field's color.
const COLOR_SQUARE: f32 = 24.;

/// What a row's control shows: its value, and whether it can be changed.
pub(super) struct RowState {
    /// The setting's value.
    pub value: Value,
    /// Whether the row's dependency is met.
    pub enabled: bool,
}

impl SettingsPage {
    /// The row's control: a stateless one drawn here, or its stateful widget.
    pub(super) fn control(
        &self,
        def: &'static SettingDef,
        state: &RowState,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let enabled = state.enabled;
        match (def.control, &state.value) {
            (Control::Toggle, Value::Bool(on)) => Toggle::new(def.key, *on, def.label)
                .disabled(!enabled)
                .on_change(cx.listener(move |this, on: &bool, _, cx| {
                    this.write(def, Value::Bool(*on), cx);
                }))
                .into_any_element(),
            (Control::Segmented(choices), Value::Choice(id)) => {
                let labels = choices.iter().map(|choice| choice.label);
                let segmented = Segmented::new(def.key, def.label, labels, index_of(def, id));
                let segmented =
                    if enabled { segmented.on_select(choice_listener(def, cx)) } else { segmented };
                div().w_full().child(segmented).into_any_element()
            }
            (Control::Swatches { choices, color }, Value::Choice(id)) => {
                let row =
                    SwatchRow { label: def.label, choices, selected: index_of(def, id), color };
                let on_select = enabled.then(|| Rc::new(choice_listener(def, cx)) as SelectHandler);
                swatches(def.key, &row, p, on_select.as_ref()).into_any_element()
            }
            (Control::Button { label, action }, _) => Button::new(def.key, label)
                .disabled(!enabled)
                .on_click(move |_, window, cx| action(window, cx))
                .into_any_element(),
            _ => self.widget(def, enabled, cx),
        }
    }

    /// The row's stateful control; a path field gets a Browse button, a color field a square.
    fn widget(
        &self,
        def: &'static SettingDef,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(element) = self.widgets.element(def) else { return div().into_any_element() };
        if self.widgets.field(def.key).is_none() {
            return element;
        }
        let field = Self::field_with_escape(def, element, cx);
        match def.control {
            Control::Path => with_browse(def, field, enabled, cx),
            Control::HexColor => with_color_square(def, field, cx),
            _ => field,
        }
    }
}

/// Saves the choice at the picked index.
fn choice_listener(
    def: &'static SettingDef,
    cx: &Context<SettingsPage>,
) -> impl Fn(&usize, &mut Window, &mut App) + 'static {
    cx.listener(move |page, index: &usize, _, cx| page.write_choice(def, *index, cx))
}

/// The position of the choice `id` among the row's choices.
fn index_of(def: &SettingDef, id: &str) -> usize {
    def.choices().iter().position(|choice| choice.id == id).unwrap_or_default()
}

/// A path field and its Browse button.
fn with_browse(
    def: &'static SettingDef,
    field: AnyElement,
    enabled: bool,
    cx: &mut Context<SettingsPage>,
) -> AnyElement {
    let browse = Button::new("browse", "Browse")
        .disabled(!enabled)
        .on_click(cx.listener(move |_, _, _, cx| browse_for(def, cx)));
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(BESIDE_GAP))
        .child(field)
        .child(browse)
        .into_any_element()
}

/// Asks for a file and saves its path for `def`.
fn browse_for(def: &'static SettingDef, cx: &mut Context<SettingsPage>) {
    let options =
        PathPromptOptions { files: true, directories: false, multiple: false, prompt: None };
    let chosen = cx.prompt_for_paths(options);
    cx.spawn(async move |page, cx| {
        let Ok(Ok(Some(paths))) = chosen.await else { return };
        let Some(path) = paths.into_iter().next() else { return };
        let text = path.to_string_lossy().into_owned();
        page.update(cx, |page, cx| page.write(def, Value::Text(text), cx)).ok();
    })
    .detach();
}

/// A color field and a square of the saved color.
fn with_color_square(
    def: &SettingDef,
    field: AnyElement,
    cx: &Context<SettingsPage>,
) -> AnyElement {
    let color = match def.value(settings::get(cx)) {
        Value::Text(text) => registry::hex_color(&text),
        _ => None,
    };
    let square = div()
        .flex_none()
        .size(px(COLOR_SQUARE))
        .rounded(px(2.))
        .when_some(color, |square, color| square.bg(gpui::rgb(color)));
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(BESIDE_GAP))
        .child(field)
        .child(square)
        .into_any_element()
}
