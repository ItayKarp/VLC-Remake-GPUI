//! The Keyboard Shortcuts dialog (S2.10.4, `dialogs/keyboard_shortcuts/keyboard_shortcuts.png`):
//! every command's shortcuts in sections, with a search over the names, the keys or both. It
//! is read-only in this stage, so there are no pencils and no Save; editing comes with S5.6.
//! There are no global hotkeys yet, so that column stays empty. It's the "full list in the
//! menu" that onboarding promises, opened from the `…` menu and the macOS app menu.

mod filter;

use gpui::{
    AnyElement, App, Context, Div, Entity, SharedString, Window, div, prelude::*, px, relative, svg,
};

use self::filter::{Field, Group, Row};
use super::MainWindow;
use crate::assets::icons;
use crate::keymap::actions::ShowKeyboardShortcuts;
use crate::theme::{self, Palette};
use crate::ui::dropdown::Dropdown;
use crate::ui::metrics;
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::text_input::{Look, TextInput};

/// The dialog's width when the window has room.
const WIDTH: f32 = 760.;
/// The field dropdown's width.
const FIELD_WIDTH: f32 = 180.;
/// A row's height, and a key well's.
const ROW_HEIGHT: f32 = 40.;
/// A key well's height.
const WELL_HEIGHT: f32 = 30.;
/// The search icon's size.
const ICON_SIZE: f32 = 18.;
/// The share of a row the Action column takes; the two key columns split the rest.
const ACTION_SHARE: f32 = 0.4;

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
                .with_placeholder("Search Keyboard Shortcuts…")
                .with_label("Search keyboard shortcuts")
        });
        let fields = Field::ALL.iter().map(|field| SharedString::from(field.label())).collect();
        let field = cx.new(|cx| Dropdown::new("Search in", fields, 0, window, cx));
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

    /// Opens the dialog over the window, listing the shortcuts as they are now.
    fn show_keyboard_shortcuts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let groups = filter::groups(cx);
        let search = Search::new(window, cx);
        let toolbar = {
            let search = search.clone();
            move |window: &mut Window, cx: &mut App| toolbar(&search, window, cx)
        };
        let body = move |window: &mut Window, cx: &mut App| {
            let p = theme::palette(window, cx);
            table(&search.results(&groups, cx), &p)
        };
        let buttons = vec![DialogButton::new("Close").primary()];
        let dialog = cx.new(|cx| {
            Dialog::new("Keyboard Shortcuts", body, buttons, cx)
                .with_width(WIDTH)
                .with_toolbar(toolbar)
        });
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

/// The search row and the column headings.
fn toolbar(search: &Search, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = theme::palette(window, cx);
    let field = div()
        .flex_1()
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(ROW_HEIGHT))
        .px(px(12.))
        .rounded(px(metrics::RADIUS))
        .bg(p.well)
        .child(svg().path(icons::SEARCH).size(px(ICON_SIZE)).flex_none().text_color(p.icon))
        .child(div().flex_1().child(search.query.clone()));
    let search_row = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(field)
        .child(div().w(px(FIELD_WIDTH)).flex_none().child(search.field.clone()));
    let heading = |text: &'static str| div().flex().text_color(p.text_muted).child(text);
    let centered = |text| heading(text).justify_center();
    let headings = columns(heading("Action"), centered("Hotkey"), centered("Global Hotkey"))
        .pt(px(16.))
        .pb(px(4.))
        .text_size(px(metrics::TEXT_SMALL));
    div().flex().flex_col().pb(px(4.)).child(search_row).child(headings).into_any_element()
}

/// The sections and their rows, or a note when nothing matches.
fn table(groups: &[Group], p: &Palette) -> AnyElement {
    if groups.is_empty() {
        return div()
            .py(px(24.))
            .text_color(p.text_muted)
            .child("No shortcuts match your search.")
            .into_any_element();
    }
    let sections = groups.iter().map(|group| {
        let title = div().pt(px(16.)).pb(px(4.)).text_color(p.text_muted).child(group.title);
        div().flex().flex_col().child(title).children(group.rows.iter().map(|row| line(row, p)))
    });
    div().flex().flex_col().pb(px(8.)).children(sections).into_any_element()
}

/// One command: its name, its keys, and the empty global hotkey.
fn line(row: &Row, p: &Palette) -> Div {
    let keys: SharedString = row.keys.join(", ").into();
    let name = div().text_color(p.text).child(row.label);
    columns(name, well(keys, p), well(SharedString::default(), p)).h(px(ROW_HEIGHT))
}

/// A read-only key field.
fn well(keys: SharedString, p: &Palette) -> Div {
    div()
        .h(px(WELL_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(metrics::RADIUS))
        .bg(p.well)
        .text_color(p.text_strong)
        .child(keys)
}

/// The three columns of a row or of the headings.
fn columns(action: Div, hotkey: Div, global: Div) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .child(action.w(relative(ACTION_SHARE)).flex_none().min_w_0())
        .child(hotkey.flex_1().min_w_0())
        .child(global.flex_1().min_w_0())
}
