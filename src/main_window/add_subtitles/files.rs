//! To This File, From Computer (S6.4.3, `add_subtitles_to_playlist_folder_02.png`): the source
//! folder's subtitle files, the ones that fit the video first, any number of them checked
//! (several languages at once). Subtitles already linked to the video come first, checked, with
//! a remove button (S6.4.7); unchecking one unlinks it on Save.

use std::ops::Range;
use std::path::PathBuf;

use gpui::{
    AnyElement, ClickEvent, Context, Div, Stateful, accesskit, div, prelude::*, px, uniform_list,
};

use super::model::Choice;
use super::render::{PADDING, ROW_GAP, ROW_HEIGHT, file_name, icon_and_name, note};
use super::{AddSubtitlesDialog, Body};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::button::IconButton;
use crate::ui::icon::icon;
use crate::ui::{focus, metrics};

/// The check's size.
const CHECK: f32 = 24.;

/// One video's candidate subtitles.
#[derive(Clone, Debug, Default)]
pub(super) struct Files {
    /// The video's linked and chosen subtitles.
    pub(super) choice: Choice,
    /// What the list shows, in order: the linked ones, then the source folder's.
    pub(super) candidates: Vec<PathBuf>,
}

impl AddSubtitlesDialog {
    /// The candidates, or a note when there are none.
    pub(super) fn render_files(files: &Files, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if files.candidates.is_empty() {
            return note(
                tr!("No subtitle files in this folder. Click the folder above to choose another."),
                p,
            )
            .into_any_element();
        }
        let list = uniform_list(
            "add-subtitles-files",
            files.candidates.len(),
            cx.processor(|this, range: Range<usize>, window, cx| {
                let p = theme::palette(window, cx);
                let Body::Files(files) = &this.body else { return Vec::new() };
                range.map(|index| this.file_row(files, index, &p, cx)).collect::<Vec<_>>()
            }),
        )
        .size_full();
        div()
            .id("add-subtitles-files-area")
            .flex_1()
            .min_h_0()
            .px(px(PADDING))
            .role(accesskit::Role::List)
            .aria_label(tr!("Subtitle files"))
            .child(list)
            .into_any_element()
    }

    /// Candidate `index`: its icon and name, then its conflict or error, the check, and for a
    /// linked one the remove button.
    fn file_row(
        &self,
        files: &Files,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let path = files.candidates[index].clone();
        let checked = files.choice.is_chosen(&path);
        let linked = files.choice.linked.contains(&path);
        let pair = (files.choice.video.clone(), path.clone());
        let status = self.pair_status(&pair, checked && !linked, p, cx);
        let toggle_path = path.clone();
        let row = div()
            .id(("add-subtitles-file", index))
            .tab_index(0)
            .key_context(focus::CONTROL)
            .role(accesskit::Role::CheckBox)
            .aria_toggled(checked.into())
            .aria_label(file_name(&path))
            .h(px(ROW_HEIGHT))
            .flex_row_start()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .ps(px(18.))
            .pe(px(16.))
            .rounded(px(metrics::RADIUS + 2.))
            .cursor_pointer()
            .text_color(if checked { p.text } else { p.text_muted })
            .when(checked, |row| row.bg(p.sunken))
            .hover(|style| style.bg(p.hover))
            .on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_file(&toggle_path, cx)),
            )
            .child(icon_and_name(icons::SUBTITLE_FILE, file_name(&path), p))
            .child(trailing(
                status,
                checked,
                linked.then(|| Self::remove_button(index, &path, cx)),
                p,
            ));
        spaced(focus::outline_ring(row, p)).into_any_element()
    }

    /// A linked subtitle's remove button: leaves it out, which unlinks it on Save.
    fn remove_button(index: usize, path: &std::path::Path, cx: &mut Context<Self>) -> IconButton {
        let path = path.to_path_buf();
        IconButton::new(("add-subtitles-unlink", index), icons::REMOVE, tr!("Remove this subtitle"))
            .size(28., 18.)
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if let Body::Files(files) = &mut this.body
                    && files.choice.is_chosen(&path)
                {
                    files.choice.toggle(&path);
                    cx.notify();
                }
            }))
    }

    /// A click on candidate `path`: checks or unchecks it.
    fn toggle_file(&mut self, path: &std::path::Path, cx: &mut Context<Self>) {
        let Body::Files(files) = &mut self.body else { return };
        files.choice.toggle(path);
        self.status = None;
        self.check_conflicts(cx);
        cx.notify();
    }
}

/// A row's right end: its conflict or error, the "Linked" tag and remove button, and the check.
fn trailing(
    status: Option<AnyElement>,
    checked: bool,
    remove: Option<IconButton>,
    p: &Palette,
) -> Div {
    let tag = remove.is_some().then(|| {
        div().text_size(text_px(metrics::TEXT_SMALL)).text_color(p.text_muted).child(tr!("Linked"))
    });
    let check = div()
        .size(px(CHECK))
        .flex_none()
        .when(checked, |slot| slot.child(icon(icons::CHECK, CHECK, p.accent_fg)));
    div()
        .flex_none()
        .flex_row_start()
        .items_center()
        .gap(px(10.))
        .children(status)
        .children(tag)
        .children(remove)
        .child(check)
}

/// A row with the space below it that separates it from the next.
pub(super) fn spaced(row: Stateful<Div>) -> Div {
    div().pb(px(ROW_GAP)).child(row)
}
