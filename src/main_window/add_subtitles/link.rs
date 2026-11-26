//! To This File, From Link (S6.4.5; no design, so it follows From OpenSubtitles): a field for an
//! `http` or `https` link to one subtitle file, which Save downloads into the app's
//! `subtitles/` folder and links to the video ([`crate::subtitle_links::download`]).

use gpui::{AnyElement, AppContext as _, Context, Entity, Window, div, prelude::*, px};

use super::AddSubtitlesDialog;
use super::render::{PADDING, note};
use crate::i18n::DirectionalStyle;
use crate::subtitle_links::download;
use crate::theme::Palette;
use crate::theme::text_px;
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
            .with_label(tr!("Subtitle file link"))
    })
}

impl AddSubtitlesDialog {
    /// What From Link does, and what the link typed names.
    pub(super) fn render_link(&self, p: &Palette, cx: &Context<Self>) -> AnyElement {
        let named = link_note(self.controls.link_field.read(cx).text());
        let explanation = note(
            "Paste a link to a subtitle file (.srt, .ass, .vtt, …). VLC keeps a copy, so the \
             subtitles stay with the video if the link stops working.",
            p,
        );
        let named = named.map(|LinkNote { text, problem }| {
            div()
                .px(px(PADDING + 6.))
                .text_size(text_px(metrics::TEXT_SMALL + 1.))
                .text_color(if problem { p.warning } else { p.text })
                .child(text)
        });
        div()
            .flex_row_start()
            .flex_col()
            .gap(px(8.))
            .child(explanation)
            .children(named)
            .into_any_element()
    }

    /// The link changed: what it names is shown again.
    pub(super) fn link_changed(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        cx.notify();
    }
}

/// What the dialog says under the link field.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LinkNote {
    text: String,
    /// Whether it says what's wrong with the link, in the warning color.
    problem: bool,
}

/// What the link `typed` names: the file Save downloads, or why it can't; nothing while the
/// field is blank.
fn link_note(typed: &str) -> Option<LinkNote> {
    let typed = typed.trim();
    if typed.is_empty() {
        return None;
    }
    let note = match download::parse_link(typed).map(|url| download::name_of(&url)) {
        Ok(Some(name)) => LinkNote {
            text: tr!("Save downloads “{name}” and adds it to this video.", name),
            problem: false,
        },
        Ok(None) => LinkNote { text: download::Failure::NotSubtitleName.message(), problem: true },
        Err(failure) => LinkNote { text: failure.message(), problem: true },
    };
    Some(note)
}

#[cfg(test)]
mod tests {
    use super::{LinkNote, link_note};

    fn problem(text: &str) -> LinkNote {
        LinkNote { text: text.to_owned(), problem: true }
    }

    #[test]
    fn a_blank_field_says_nothing() {
        assert_eq!(link_note("  "), None);
    }

    #[test]
    fn a_subtitle_link_names_the_file_save_downloads() {
        let note = link_note(" https://example.com/subs/The.Wand.en.srt ");
        let text = "Save downloads “The.Wand.en.srt” and adds it to this video.";
        assert_eq!(note, Some(LinkNote { text: text.to_owned(), problem: false }));
    }

    #[test]
    fn other_links_say_what_is_wrong() {
        let not_subtitles = "The link doesn't name a subtitle file (.srt, .ass, .vtt, …).";
        assert_eq!(link_note("https://example.com/page"), Some(problem(not_subtitles)));
        let not_a_link = "Enter a link that starts with http:// or https://.";
        assert_eq!(link_note("ftp://example.com/a.srt"), Some(problem(not_a_link)));
    }
}
