//! The Keyboard Shortcuts dialog (S2.10.4, S5.6.3, `dialogs/keyboard_shortcuts/keyboard_shortcuts.png`):
//! every command's shortcuts in sections, with a search over the names, the keys or both. A
//! pencil records a row's key (S5.6.4); a key another command has asks to Replace (S5.6.5);
//! a changed row can be reset (S5.6.6). Save binds the edits; Cancel drops them, and Escape or
//! ✕ asks first when there are any. Global hotkeys aren't supported yet, so that column stays
//! empty. It's the "full list in the menu" that onboarding promises, opened from the `…` menu,
//! the macOS app menu and Settings → Controls.

mod editor;
mod filter;
mod rows;
#[cfg(test)]
mod screenshots;

use gpui::{
    AnyElement, App, Context, DismissEvent, Div, Entity, PromptLevel, SharedString, WeakEntity,
    Window, div, prelude::*, px, svg,
};

use self::editor::ShortcutsEditor;
use self::filter::{Field, Group};
use self::rows::{ROW_HEIGHT, columns, table};
use super::MainWindow;
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::keymap::{self, actions::ShowKeyboardShortcuts};
use crate::settings;
use crate::theme;
use crate::theme::text_px;
use crate::ui::dropdown::Dropdown;
use crate::ui::metrics;
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::text_input::{Look, TextInput};

/// The dialog's width when the window has room, as in the design.
const WIDTH: f32 = 1000.;
/// The field dropdown's width.
const FIELD_WIDTH: f32 = 180.;
/// The search icon's size.
const ICON_SIZE: f32 = 18.;

/// The search field and the field dropdown.
#[derive(Clone)]
struct Search {
    query: Entity<TextInput>,
    field: Entity<Dropdown>,
}

impl Search {
    fn new(window: &mut Window, cx: &mut App) -> Self {
        let query = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_look(Look::Bare)
                .with_placeholder(tr!("Search Keyboard Shortcuts…"))
                .with_label(tr!("Search keyboard shortcuts"))
        });
        let fields = Field::ALL.iter().map(|field| SharedString::from(field.label())).collect();
        let field = cx.new(|cx| Dropdown::new(tr!("Search in"), fields, 0, window, cx));
        Self { query, field }
    }

    /// The groups that answer the search as it stands.
    fn results(&self, groups: &[Group], cx: &App) -> Vec<Group> {
        let field = Field::at(self.field.read(cx).selected());
        filter::search(groups, self.query.read(cx).text(), field)
    }
}

impl MainWindow {
    /// Adds the handler that opens the dialog.
    pub(super) fn shortcuts_dialog_action(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &ShowKeyboardShortcuts, window, cx| {
            this.show_keyboard_shortcuts(window, cx);
        }))
    }

    /// Opens the dialog over the window, editing the shortcuts as they are now.
    fn show_keyboard_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shortcuts = keymap::load(cx);
        let editor = cx.new(|_| ShortcutsEditor::new(shortcuts));
        let dialog = build(editor, window, cx);
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

/// The dialog around `editor`.
fn build(editor: Entity<ShortcutsEditor>, window: &mut Window, cx: &mut App) -> Entity<Dialog> {
    let search = Search::new(window, cx);
    let toolbar = {
        let search = search.clone();
        move |window: &mut Window, cx: &mut App| toolbar(&search, window, cx)
    };
    let body = {
        let editor = editor.clone();
        move |window: &mut Window, cx: &mut App| {
            let p = theme::palette(window, cx);
            let groups = filter::groups(editor.read(cx).draft());
            table(&search.results(&groups, cx), &editor, &p, cx)
        }
    };
    let save = {
        let editor = editor.clone();
        DialogButton::new(tr!("Save")).primary().on_click(move |_, cx| {
            let draft = editor.read(cx).draft().clone();
            keymap::apply(&draft, cx);
        })
    };
    let buttons = vec![DialogButton::new(tr!("Cancel")).leading(), save];
    cx.new(|cx| {
        let this = cx.weak_entity();
        Dialog::new(tr!("Keyboard Shortcuts"), body, buttons, cx)
            .with_width(WIDTH)
            .with_toolbar(toolbar)
            .with_close_button()
            .with_close_guard(move |window, cx| may_close(&editor, &this, window, cx))
    })
}

/// Opens the dialog in a main window, bringing it forward ("Edit Keyboard Shortcuts" in
/// Settings → Controls, which has no dialog layer of its own).
pub fn edit_keyboard_shortcuts(_: &mut Window, cx: &mut App) {
    let main = cx.windows().into_iter().find_map(|handle| handle.downcast::<MainWindow>());
    let Some(main) = main else {
        tracing::warn!("no main window to edit the keyboard shortcuts in");
        return;
    };
    cx.defer(move |cx| {
        main.update(cx, |main, window, cx| {
            window.activate_window();
            main.show_keyboard_shortcuts(window, cx);
        })
        .ok();
    });
}

/// Whether Escape or ✕ may close the dialog now: yes without edits (or with confirmations
/// off); otherwise it asks, and closes it on Discard Changes.
fn may_close(
    editor: &Entity<ShortcutsEditor>,
    dialog: &WeakEntity<Dialog>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if !editor.read(cx).is_dirty() || !settings::get(cx).prefs.startup.show_dialogs {
        return true;
    }
    let detail = tr!("The shortcuts you changed won't be saved.");
    let answer = window.prompt(
        PromptLevel::Warning,
        tr!("Discard your changes?"),
        Some(detail),
        &[tr!("Discard Changes"), tr!("Keep Editing")],
        cx,
    );
    let dialog = dialog.clone();
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            dialog.update(cx, |_, cx| cx.emit(DismissEvent)).ok();
        }
    })
    .detach();
    false
}

/// The search row and the column headings.
fn toolbar(search: &Search, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = theme::palette(window, cx);
    let field = div()
        .flex_1()
        .flex_row_start()
        .items_center()
        .gap(px(10.))
        .h(px(ROW_HEIGHT))
        .px(px(12.))
        .rounded(px(metrics::RADIUS))
        .bg(p.well)
        .child(svg().path(icons::SEARCH).size(px(ICON_SIZE)).flex_none().text_color(p.icon))
        .child(div().flex_1().child(search.query.clone()));
    let search_row = div()
        .flex_row_start()
        .items_center()
        .gap(px(12.))
        .child(field)
        .child(div().w(px(FIELD_WIDTH)).flex_none().child(search.field.clone()));
    let heading = |text: &'static str| div().flex_row_start().text_color(p.text_muted).child(text);
    let centered = |text| heading(text).justify_center();
    let headings =
        columns(heading(tr!("Action")), centered(tr!("Hotkey")), centered(tr!("Global Hotkey")))
            .pt(px(16.))
            .pb(px(4.))
            .text_size(text_px(metrics::TEXT_SMALL));
    div()
        .flex_row_start()
        .flex_col()
        .pb(px(4.))
        .child(search_row)
        .child(headings)
        .into_any_element()
}
