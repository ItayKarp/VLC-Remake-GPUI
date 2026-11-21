//! Step 1: the welcome text and the media folder picker.

use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, FontWeight, PathPromptOptions, SharedString, div, prelude::*,
    px, svg,
};

use super::Onboarding;
use super::layout::bottom_bar;
use crate::assets::icons;
use crate::settings;
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};

/// Width of the text column under the title.
const COLUMN_WIDTH: f32 = 594.;

/// What VLC is, shown under the welcome title.
const INTRO: &str = "VLC is a free and open source cross-platform multimedia player and \
                     framework that plays most multimedia files as well as DVDs, Audio CDs, \
                     VCDs, and various streaming protocols.";

impl Onboarding {
    /// Renders step 1: logo, welcome title, intro text and the folder picker.
    pub(super) fn render_welcome(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let folder = SharedString::from(settings::display_path(&self.media_folder));

        div()
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .pt(px(118.))
            .child(svg().path(icons::LOGO).size(px(140.)).text_color(p.accent))
            .child(welcome_title(p))
            .child(
                div()
                    .w(px(COLUMN_WIDTH))
                    .flex()
                    .flex_col()
                    .child(intro(p))
                    .child(folder_prompt(p))
                    .child(folder_picker(folder, p, cx)),
            )
            .child(bottom_bar().justify_end().child(self.nav_buttons(false, "Next", cx)))
            .into_any_element()
    }

    /// Asks the OS for a folder and, if one is chosen, makes it the media folder.
    fn browse(cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose Media Folder".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            let Some(folder) = paths.into_iter().next() else { return };
            this.update(cx, |this, cx| this.set_media_folder(folder, cx)).ok();
        })
        .detach();
    }

    fn set_media_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        self.media_folder = folder;
        cx.notify();
    }
}

fn welcome_title(p: &Palette) -> impl IntoElement {
    div()
        .mt(px(18.))
        .text_size(px(36.))
        .line_height(px(44.))
        .font_weight(FontWeight::LIGHT)
        .text_color(p.text_strong)
        .child("Welcome to VLC!")
}

fn intro(p: &Palette) -> impl IntoElement {
    div().mt(px(34.)).text_size(px(16.)).line_height(px(28.)).text_color(p.text_muted).child(INTRO)
}

fn folder_prompt(p: &Palette) -> impl IntoElement {
    div()
        .mt(px(50.))
        .text_size(px(15.))
        .line_height(px(20.))
        .text_color(p.text_muted)
        .child("Choose a Media Folder to get started with")
}

/// The chosen folder and a Browse button; clicking either opens the folder dialog.
fn folder_picker(
    folder: SharedString,
    p: &Palette,
    cx: &mut Context<Onboarding>,
) -> impl IntoElement {
    div()
        .mt(px(14.))
        .flex()
        .flex_row()
        .gap(px(11.))
        .h(px(35.))
        .child(folder_field(folder, p, cx))
        .child(
            Button::new("browse", "Browse")
                .style(ButtonStyle::Filled)
                .width(114.)
                .on_click(cx.listener(|_, _: &ClickEvent, _, cx| Onboarding::browse(cx))),
        )
}

fn folder_field(
    folder: SharedString,
    p: &Palette,
    cx: &mut Context<Onboarding>,
) -> impl IntoElement {
    div()
        .id("media-folder")
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .px(px(13.))
        .rounded(px(4.))
        .bg(p.sunken)
        .border_2()
        .border_color(p.border)
        .text_size(px(16.))
        .text_color(p.text_strong)
        .overflow_hidden()
        .cursor_pointer()
        .on_click(cx.listener(|_, _: &ClickEvent, _, cx| Onboarding::browse(cx)))
        .child(div().truncate().child(folder))
}
