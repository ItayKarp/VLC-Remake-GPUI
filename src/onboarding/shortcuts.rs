//! Step 3: the basic keyboard shortcuts, drawn as key caps, and the Finish row.
//! The keys come from the keymap (S0.5.3), so this page always teaches the
//! shortcuts that work.

use gpui::{Action, AnyElement, App, Context, div, prelude::*, px, svg};

use super::finish::FinishState;
use super::layout::{EDGE_LEFT, bottom_bar};
use super::{Onboarding, save_error};
use crate::assets::icons;
use crate::keymap::actions as act;
use crate::keymap::key_caps;
use crate::keymap::label::{Arrow, KeyCap};
use crate::theme::Palette;
use crate::theme::text_px;

/// A row: a label and the commands whose shortcuts it shows, separated by "/".
struct Shortcut {
    label: &'static str,
    commands: &'static [&'static dyn Action],
}

const LEFT_SHORTCUTS: &[Shortcut] = &[
    Shortcut { label: "Play / Pause", commands: &[&act::PlayPause] },
    Shortcut {
        label: "Seek Backward / Forward",
        commands: &[&act::SeekBackward, &act::SeekForward],
    },
    Shortcut { label: "Previous / Next Item", commands: &[&act::PreviousItem, &act::NextItem] },
    Shortcut { label: "Previous / Next Frame", commands: &[&act::PreviousFrame, &act::NextFrame] },
    Shortcut { label: "Cycle Subtitle Track", commands: &[&act::CycleSubtitleTrack] },
    Shortcut { label: "Cycle Audio Track", commands: &[&act::CycleAudioTrack] },
];

const RIGHT_SHORTCUTS: &[Shortcut] = &[
    Shortcut { label: "Toggle Fullscreen", commands: &[&act::ToggleFullscreen] },
    Shortcut { label: "Increase / Decrease Volume", commands: &[&act::VolumeUp, &act::VolumeDown] },
    Shortcut { label: "Show / Hide Queue", commands: &[&act::ToggleQueue] },
    Shortcut { label: "Show / Hide File Browser", commands: &[&act::ToggleFileBrowser] },
    Shortcut { label: "Take a Snapshot", commands: &[&act::Snapshot] },
    Shortcut { label: "Record", commands: &[&act::Record] },
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
                    .text_size(text_px(26.))
                    .line_height(text_px(34.))
                    .text_color(p.text)
                    .child("Learn the basic keyboard shortcuts."),
            )
            .child(
                div()
                    .mt(px(55.))
                    .px(px(EDGE_LEFT))
                    .flex()
                    .flex_row()
                    .child(shortcut_column(LEFT_SHORTCUTS, p, cx))
                    .child(shortcut_column(RIGHT_SHORTCUTS, p, cx)),
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
                    .text_size(text_px(16.))
                    .text_color(p.text_muted)
                    .child("Find the full list in the menu."),
            )
            .child(self.nav_buttons(true, finish_label, cx))
            .into_any_element()
    }
}

fn shortcut_column(shortcuts: &'static [Shortcut], p: &Palette, cx: &App) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap(px(37.))
        .children(shortcuts.iter().map(|s| shortcut_row(s, p, cx)))
}

fn shortcut_row(shortcut: &Shortcut, p: &Palette, cx: &App) -> impl IntoElement {
    let mut keys = div().flex().flex_row().items_center().gap(px(10.));
    for (i, action) in shortcut.commands.iter().enumerate() {
        if i > 0 {
            keys = keys.child(div().text_size(text_px(16.)).text_color(p.text_muted).child("/"));
        }
        let caps = key_caps(*action, cx);
        keys = keys.child(
            div()
                .flex()
                .flex_row()
                .gap(px(6.))
                .children(caps.into_iter().map(|cap| keycap(cap, p))),
        );
    }

    div()
        .h(px(45.))
        .flex()
        .flex_row()
        .items_center()
        .child(
            div()
                .w(px(240.))
                .text_size(text_px(16.))
                .text_color(p.text_muted)
                .child(shortcut.label),
        )
        .child(keys)
}

fn keycap(cap: KeyCap, p: &Palette) -> impl IntoElement {
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
        .text_size(text_px(16.))
        .text_color(p.text_strong);

    let face = match cap {
        KeyCap::Text(text) => face.child(text),
        KeyCap::Arrow(arrow) => {
            face.child(svg().path(arrow_icon(arrow)).size(px(20.)).text_color(p.text_strong))
        }
    };

    // The lip under the key cap gives it the raised look from the design.
    div().pb(px(5.)).rounded(px(9.)).bg(p.key_edge).child(face)
}

fn arrow_icon(arrow: Arrow) -> &'static str {
    match arrow {
        Arrow::Left => icons::ARROW_LEFT,
        Arrow::Right => icons::ARROW_RIGHT,
        Arrow::Up => icons::ARROW_UP,
        Arrow::Down => icons::ARROW_DOWN,
    }
}
