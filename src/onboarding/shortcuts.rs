//! Step 3: the basic keyboard shortcuts, drawn as key caps, and the Finish row.

use gpui::{AnyElement, Context, div, prelude::*, px, svg};

use super::finish::FinishState;
use super::layout::{EDGE_LEFT, bottom_bar};
use super::{Onboarding, save_error};
use crate::assets::icons;
use crate::theme::Palette;

/// One key on a key cap: a label or an icon (arrow keys).
#[derive(Clone, Copy)]
enum Key {
    Text(&'static str),
    Icon(&'static str),
}

/// A shortcut: alternatives are separated by "/", keys within one
/// alternative are pressed together (e.g. SHIFT + S).
struct Shortcut {
    label: &'static str,
    keys: &'static [&'static [Key]],
}

const LEFT_SHORTCUTS: &[Shortcut] = &[
    Shortcut { label: "Play / Pause", keys: &[&[Key::Text("Space")]] },
    Shortcut {
        label: "Seek Backward / Forward",
        keys: &[&[Key::Icon(icons::ARROW_LEFT)], &[Key::Icon(icons::ARROW_RIGHT)]],
    },
    Shortcut { label: "Previous / Next Item", keys: &[&[Key::Text("<")], &[Key::Text(">")]] },
    Shortcut { label: "Previous / Next Frame", keys: &[&[Key::Text(",")], &[Key::Text(".")]] },
    Shortcut { label: "Cycle Subtitle Track", keys: &[&[Key::Text("V")]] },
    Shortcut { label: "Cycle Audio Track", keys: &[&[Key::Text("B")]] },
];

const RIGHT_SHORTCUTS: &[Shortcut] = &[
    Shortcut { label: "Toggle Fullscreen", keys: &[&[Key::Text("F")]] },
    Shortcut {
        label: "Increase / Decrease Volume",
        keys: &[&[Key::Icon(icons::ARROW_UP)], &[Key::Icon(icons::ARROW_DOWN)]],
    },
    Shortcut { label: "Show / Hide Queue", keys: &[&[Key::Text("Q")]] },
    Shortcut { label: "Show / Hide File Browser", keys: &[&[Key::Text("L")]] },
    Shortcut { label: "Take a Snapshot", keys: &[&[Key::Text("SHIFT"), Key::Text("S")]] },
    Shortcut { label: "Record", keys: &[&[Key::Text("SHIFT"), Key::Text("R")]] },
];

impl Onboarding {
    /// Renders step 3: the heading, two columns of shortcuts and the Finish row.
    pub(super) fn render_shortcuts(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .mt(px(64.))
                    .w_full()
                    .text_center()
                    .text_size(px(26.))
                    .line_height(px(34.))
                    .text_color(p.text)
                    .child("Learn the basic keyboard shortcuts."),
            )
            .child(
                div()
                    .mt(px(55.))
                    .px(px(EDGE_LEFT))
                    .flex()
                    .flex_row()
                    .child(shortcut_column(LEFT_SHORTCUTS, p))
                    .child(shortcut_column(RIGHT_SHORTCUTS, p)),
            )
            .child(self.finish_row(p, cx))
            .into_any_element()
    }

    /// The hint and Back / Finish, or the save error if the last Finish failed.
    fn finish_row(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if let Some(error) = self.save_error() {
            return save_error::render(error, p, cx).into_any_element();
        }
        let finish_label = if self.finish == FinishState::Saving { "Saving…" } else { "Finish" };
        bottom_bar()
            .left(px(EDGE_LEFT))
            .justify_between()
            .child(
                div()
                    .text_size(px(16.))
                    .text_color(p.text_muted)
                    .child("Find the full list in the menu."),
            )
            .child(self.nav_buttons(true, finish_label, cx))
            .into_any_element()
    }
}

fn shortcut_column(shortcuts: &'static [Shortcut], p: &Palette) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(37.))
        .children(shortcuts.iter().map(|s| shortcut_row(s, p)))
}

fn shortcut_row(shortcut: &Shortcut, p: &Palette) -> impl IntoElement {
    let mut keys = div().flex().flex_row().items_center().gap(px(10.));
    for (i, combo) in shortcut.keys.iter().enumerate() {
        if i > 0 {
            keys = keys.child(div().text_size(px(16.)).text_color(p.text_muted).child("/"));
        }
        keys = keys.child(
            div().flex().flex_row().gap(px(6.)).children(combo.iter().map(|key| keycap(*key, p))),
        );
    }

    div()
        .h(px(45.))
        .flex()
        .flex_row()
        .items_center()
        .child(div().w(px(240.)).text_size(px(16.)).text_color(p.text_muted).child(shortcut.label))
        .child(keys)
}

fn keycap(key: Key, p: &Palette) -> impl IntoElement {
    let face = div()
        .h(px(39.))
        .min_w(px(46.))
        .px(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .bg(p.key_face)
        .border_2()
        .border_color(p.key_border)
        .text_size(px(16.))
        .text_color(p.text_strong);

    let face = match key {
        Key::Text(text) => face.child(text),
        Key::Icon(path) => face.child(svg().path(path).size(px(20.)).text_color(p.text_strong)),
    };

    // The lip under the key cap gives it the raised look from the design.
    div().pb(px(5.)).rounded(px(9.)).bg(p.key_edge).child(face)
}
