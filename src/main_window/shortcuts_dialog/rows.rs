//! Drawing the Keyboard Shortcuts dialog's sections and rows: each command's name with a
//! reset button once changed, its key well with a pencil, the empty Global Hotkey well, and the
//! note under a row being edited (S5.6.3–S5.6.6).

use gpui::{AnyElement, App, Div, Entity, SharedString, Window, div, prelude::*, px, relative};

use super::editor::{Note, ShortcutsEditor};
use super::filter::{Group, Row};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::metrics;

/// A row's height.
pub const ROW_HEIGHT: f32 = 40.;
/// A key well's height.
const WELL_HEIGHT: f32 = 30.;
/// The share of a row the Action column takes; the two key columns split the rest.
const ACTION_SHARE: f32 = 0.4;
/// The pencil's and the reset button's side.
const SMALL_BUTTON: f32 = 24.;
/// Their icons' size.
const SMALL_ICON: f32 = 14.;

/// The sections and their rows, or a note when nothing matches.
pub fn table(
    groups: &[Group],
    editor: &Entity<ShortcutsEditor>,
    p: &Palette,
    cx: &App,
) -> AnyElement {
    if groups.is_empty() {
        return div()
            .py(px(24.))
            .text_color(p.text_muted)
            .child(tr!("No shortcuts match your search."))
            .into_any_element();
    }
    let sections = groups.iter().map(|group| {
        let title = div().pt(px(16.)).pb(px(4.)).text_color(p.text_muted).child(group.title);
        let rows = group.rows.iter().map(|row| line(row, editor, p, cx));
        div().flex_row_start().flex_col().child(title).children(rows)
    });
    div().flex_row_start().flex_col().pb(px(8.)).children(sections).into_any_element()
}

/// The three columns of a row or of the headings.
pub fn columns(action: Div, hotkey: Div, global: Div) -> Div {
    div()
        .flex_row_start()
        .items_center()
        .gap(px(16.))
        .child(action.w(relative(ACTION_SHARE)).flex_none().min_w_0())
        .child(hotkey.flex_1().min_w_0())
        .child(global.flex_1().min_w_0())
}

/// One command, and the note under it while there is one.
fn line(row: &Row, editor: &Entity<ShortcutsEditor>, p: &Palette, cx: &App) -> Div {
    let state = editor.read(cx);
    let recording = state.recording() == Some(row.index);
    let note = state.note(row.index).cloned();
    let keys =
        columns(name(row, editor, p), hotkey(row, recording, editor, p), well(p)).h(px(ROW_HEIGHT));
    div()
        .flex_row_start()
        .flex_col()
        .child(keys)
        .children(note.map(|note| note_line(row.index, note, editor, p)))
}

/// The command's name, with a reset button once its keys differ from the defaults.
fn name(row: &Row, editor: &Entity<ShortcutsEditor>, p: &Palette) -> Div {
    let reset = row.changed.then(|| {
        let (editor, index) = (editor.clone(), row.index);
        IconButton::new(("reset", row.index), icons::REPLAY, tr!("Reset to default"))
            .size(SMALL_BUTTON, SMALL_ICON)
            .on_click(move |_, _, cx| editor.update(cx, |editor, cx| editor.reset(index, cx)))
    });
    div()
        .flex_row_start()
        .items_center()
        .gap(px(6.))
        .text_color(p.text)
        .child(div().min_w_0().truncate().child(row.label))
        .children(reset)
}

/// The command's keys with a pencil; "Press a shortcut…" while recording.
fn hotkey(row: &Row, recording: bool, editor: &Entity<ShortcutsEditor>, p: &Palette) -> Div {
    let text: SharedString =
        if recording { tr!("Press a shortcut…").into() } else { row.keys.join(", ").into() };
    let start = {
        let (editor, index) = (editor.clone(), row.index);
        move |window: &mut Window, cx: &mut App| {
            editor.update(cx, |editor, cx| editor.record(index, window, cx));
        }
    };
    let pencil = IconButton::new(
        ("edit", row.index),
        icons::EDIT,
        tr!("Edit {command}", command = row.label),
    )
    .size(SMALL_BUTTON, SMALL_ICON)
    .selected(recording)
    .on_click(move |_, window, cx| start(window, cx));
    let well = well(p)
        .relative()
        .child(div().text_color(if recording { p.text_muted } else { p.text_strong }).child(text));
    let edge = if recording { p.accent } else { gpui::transparent_black() };
    let well = well.border_1().border_color(edge);
    well.child(div().absolute().inset_end(px(4.)).child(pencil))
}

/// A key field.
fn well(p: &Palette) -> Div {
    div()
        .h(px(WELL_HEIGHT))
        .flex_row_start()
        .items_center()
        .justify_center()
        .rounded(px(metrics::RADIUS))
        .bg(p.well)
}

/// The line under a row: why a key was refused, or which command has it already.
fn note_line(index: usize, note: Note, editor: &Entity<ShortcutsEditor>, p: &Palette) -> Div {
    let line = div().flex_row_start().items_center().justify_end().gap(px(8.)).pb(px(8.));
    let message = |text: SharedString| div().text_size(text_px(metrics::TEXT_SMALL)).child(text);
    match note {
        Note::Refused(text) => line.text_color(p.text_muted).child(message(text.into())),
        Note::Taken(_, conflict) => {
            let replace = {
                let editor = editor.clone();
                Button::new(("replace", index), tr!("Replace")).style(ButtonStyle::Filled).on_click(
                    move |_, _, cx| editor.update(cx, |editor, cx| editor.replace(index, cx)),
                )
            };
            let cancel = {
                let editor = editor.clone();
                Button::new(("keep", index), tr!("Cancel"))
                    .style(ButtonStyle::Ghost)
                    .on_click(move |_, _, cx| editor.update(cx, ShortcutsEditor::dismiss_note))
            };
            let text = div()
                .flex_row_start()
                .gap(px(4.))
                .text_size(text_px(metrics::TEXT_SMALL))
                .child(tr!("Already used by"))
                .child(div().italic().child(conflict.label()));
            line.text_color(p.text).child(text).child(replace).child(cancel)
        }
    }
}
