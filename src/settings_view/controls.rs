//! Each row's control: the stateless ones (toggle, segmented, swatches, button) drawn from the
//! settings on every render, and the stateful widgets with what goes beside them (a path's Browse
//! button, a color's square and palette).

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{AnyElement, App, Context, PathPromptOptions, PromptLevel, Window, div, prelude::*, px};

use super::SettingsPage;
use super::swatches::{SelectHandler, SwatchRow, swatches};
use crate::i18n;
use crate::i18n::DirectionalStyle;
use crate::settings::{
    self,
    registry::{Control, PathPick, SettingDef, Value},
};
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::segmented::Segmented;
use crate::ui::toggle::Toggle;

/// The space between a field and its Browse button or color square.
const BESIDE_GAP: f32 = 8.;

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
            (Control::Toggle, Value::Bool(on)) => Toggle::new(def.key, *on, i18n::text(def.label))
                .disabled(!enabled)
                .on_change(cx.listener(move |this, on: &bool, window, cx| {
                    this.toggle(def, *on, window, cx);
                }))
                .into_any_element(),
            (Control::Segmented(choices), Value::Choice(id)) => {
                let labels = choices.iter().map(|choice| i18n::text(choice.label));
                let label = i18n::text(def.label);
                let segmented = Segmented::new(def.key, label, labels, index_of(def, id));
                let segmented =
                    if enabled { segmented.on_select(choice_listener(def, cx)) } else { segmented };
                div().w_full().child(segmented).into_any_element()
            }
            (Control::Swatches { choices, color }, Value::Choice(id)) => {
                let label = i18n::text(def.label);
                let row = SwatchRow { label, choices, selected: index_of(def, id), color };
                let on_select = enabled.then(|| Rc::new(choice_listener(def, cx)) as SelectHandler);
                swatches(def.key, &row, p, on_select.as_ref()).into_any_element()
            }
            (Control::Button { label, action }, _) => Button::new(def.key, i18n::text(label))
                .disabled(!enabled)
                .on_click(move |_, window, cx| action(window, cx))
                .into_any_element(),
            _ => self.widget(def, enabled, cx),
        }
    }

    /// Saves a toggle, asking first when the row wants turning it off confirmed (S5.10.1),
    /// unless confirmations are off (S5.3.3).
    fn toggle(
        &mut self,
        def: &'static SettingDef,
        on: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let confirm =
            def.confirm_off.filter(|_| !on && settings::get(cx).prefs.startup.show_dialogs);
        let Some(confirm) = confirm else {
            self.write(def, Value::Bool(on), cx);
            return;
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            i18n::text(confirm.title),
            Some(i18n::text(confirm.detail)),
            &[i18n::text(confirm.button), tr!("Cancel")],
            cx,
        );
        cx.spawn(async move |page, cx| {
            if answer.await == Ok(0) {
                page.update(cx, |page, cx| page.write(def, Value::Bool(false), cx)).ok();
            }
        })
        .detach();
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
        match (def.control, def.side_button) {
            (Control::Path(pick), _) => with_browse(def, pick, field, enabled, cx),
            (Control::HexColor, _) => self.with_color_square(def, field),
            (_, Some(side)) => {
                let button = Button::new(def.key, i18n::text(side.label))
                    .disabled(!enabled)
                    .on_click(move |_, window, cx| (side.action)(window, cx));
                beside(field, button.into_any_element())
            }
            _ => field,
        }
    }
}

impl SettingsPage {
    /// A color field and the square that shows its color and opens the palette.
    fn with_color_square(&self, def: &SettingDef, field: AnyElement) -> AnyElement {
        let square = self.widgets.picker(def.key).cloned();
        div()
            .w_full()
            .flex_row_start()
            .items_center()
            .gap(px(BESIDE_GAP))
            .child(div().flex_1().min_w_0().child(field))
            .children(square)
            .into_any_element()
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
    pick: PathPick,
    field: AnyElement,
    enabled: bool,
    cx: &mut Context<SettingsPage>,
) -> AnyElement {
    let browse = Button::new("browse", tr!("Browse"))
        .disabled(!enabled)
        .on_click(cx.listener(move |_, _, _, cx| browse_for(def, pick, cx)));
    beside(field, browse.into_any_element())
}

/// A field taking the row's width with `button` after it.
fn beside(field: AnyElement, button: AnyElement) -> AnyElement {
    div()
        .w_full()
        .flex_row_start()
        .items_start()
        .gap(px(BESIDE_GAP))
        .child(div().flex_1().min_w_0().child(field))
        .child(div().flex_none().child(button))
        .into_any_element()
}

/// Asks for a file, or where to save one, and saves its path for `def`. A path the setting
/// refuses shows why under the field.
fn browse_for(def: &'static SettingDef, pick: PathPick, cx: &mut Context<SettingsPage>) {
    let chosen = match pick {
        PathPick::ExistingFile | PathPick::Folder => {
            let folder = pick == PathPick::Folder;
            let options = PathPromptOptions {
                files: !folder,
                directories: folder,
                multiple: false,
                prompt: None,
            };
            let paths = cx.prompt_for_paths(options);
            cx.background_spawn(async move {
                paths.await.ok()?.ok()?.and_then(|paths| paths.into_iter().next())
            })
        }
        PathPick::NewFile { name } => {
            let start = start_folder(def, cx);
            let path = cx.prompt_for_new_path(&start, Some(name));
            cx.background_spawn(async move { path.await.ok()?.ok()? })
        }
    };
    cx.spawn(async move |page, cx| {
        let Some(path) = chosen.await else { return };
        let text = path.to_string_lossy().into_owned();
        page.update(cx, |page, cx| page.commit_text(def, text, cx)).ok();
    })
    .detach();
}

/// Where a save dialog starts: the folder of the saved path, else the home folder.
fn start_folder(def: &SettingDef, cx: &App) -> PathBuf {
    let saved = match def.value(settings::get(cx)) {
        Value::Text(text) => PathBuf::from(text.trim()),
        _ => PathBuf::new(),
    };
    saved
        .parent()
        .filter(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .or_else(dirs::home_dir)
        .unwrap_or_default()
}
