//! To This File, From Link (S6.4.5; no design, so it follows From OpenSubtitles): a field for an
//! `http` or `https` link to one subtitle file, which Save downloads into the app's
//! `subtitles/` folder and links to the video ([`crate::subtitle_links::download`]).

use gpui::{AnyElement, AppContext as _, Context, Entity, Window, div, prelude::*, px};

use super::AddSubtitlesDialog;
use super::render::{PADDING, note};
use crate::subtitle_links::download;
use crate::theme::Palette;
use crate::ui::metrics;
use crate::ui::text_input::{Look, TextInput};

/// The link field.
pub(super) fn field(
    window: &mut Window,
    cx: &mut Context<AddSubtitlesDialog>,
) -> Entity<TextInput> {
    cx.new(|cx| {
        TextInput::new(window, cx)
            .with_look(Look::Bare)
            .with_placeholder("https://example.com/The.Wand.S01E01.en.srt")
            .with_label("Subtitle file link")
    })
}

impl AddSubtitlesDialog {
    /// What From Link does, and what the link typed names.
    pub(super) fn render_link(&self, p: &Palette, cx: &Context<Self>) -> AnyElement {
        let text = self.controls.link_field.read(cx).text().trim().to_owned();
        let named = match download::parse_link(&text).map(|url| download::name_of(&url)) {
            _ if text.is_empty() => None,
            Ok(Some(name)) => {
                Some((format!("Save downloads “{name}” and adds it to this video."), false))
            }
            Ok(None) => Some((download::Failure::NotSubtitleName.message(), true)),
            Err(failure) => Some((failure.message(), true)),
        };
        let explanation = note(
            "Paste a link to a subtitle file (.srt, .ass, .vtt, …). VLC keeps a copy, so the \
             subtitles stay with the video if the link stops working.",
            p,
        );
        let named = named.map(|(text, problem)| {
            div()
                .px(px(PADDING + 6.))
                .text_size(px(metrics::TEXT_SMALL + 1.))
                .text_color(if problem { p.warning } else { p.text })
                .child(text)
        });
        div().flex().flex_col().gap(px(8.)).child(explanation).children(named).into_any_element()
    }

    /// The link changed: what it names is shown again.
    pub(super) fn link_changed(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        cx.notify();
    }
}
