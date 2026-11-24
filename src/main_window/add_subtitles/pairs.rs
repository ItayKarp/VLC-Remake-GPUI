//! To This Folder and To This Playlist, From Computer (S6.4.2,
//! `add_subtitles_to_playlist_folder_01.png`): each video, a dashed connector, and the subtitles
//! paired with it, or "No match". A click on the subtitle side opens a picker to check others,
//! clear the row or choose a file elsewhere (the design has no way to fix a pairing; this is
//! it). When nothing in the source folder fits any video, the list says so instead of pairing
//! wrongly.

use std::ops::Range;
use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, ClickEvent, Context, DismissEvent, Div, MouseButton, MouseDownEvent,
    PathPromptOptions, Pixels, Point, SharedString, Stateful, Window, accesskit, div, prelude::*,
    px, uniform_list,
};

use super::files::spaced;
use super::model::Choice;
use super::render::{PADDING, ROW_HEIGHT, file_name, icon_and_name, note};
use super::{AddSubtitlesDialog, Body};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::subtitle_links::pairing;
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::focus;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::metrics;

/// The connector's dash color's opacity against the muted text.
const CONNECTOR_OPACITY: f32 = 0.5;

/// Every video's pairing.
#[derive(Clone, Debug, Default)]
pub(super) struct Pairs {
    /// Each video's linked and chosen subtitles, in the target's order.
    pub(super) choices: Vec<Choice>,
    /// The source folder's subtitle files, for the pickers.
    pub(super) candidates: Vec<PathBuf>,
    /// Whether any video got a subtitle from the source folder by itself.
    pub(super) paired_any: bool,
}

impl AddSubtitlesDialog {
    /// The pairs, under a note when the source folder fits none of them.
    pub(super) fn render_pairs(pairs: &Pairs, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if pairs.choices.is_empty() {
            return note(tr!("There are no videos here to add subtitles to."), p)
                .into_any_element();
        }
        let list = uniform_list(
            "add-subtitles-pairs",
            pairs.choices.len(),
            cx.processor(|this, range: Range<usize>, window, cx| {
                let p = theme::palette(window, cx);
                let Body::Pairs(pairs) = &this.body else { return Vec::new() };
                range
                    .map(|index| this.pair_row(&pairs.choices[index], index, &p, cx))
                    .collect::<Vec<_>>()
            }),
        )
        .size_full();
        div()
            .flex_1()
            .min_h_0()
            .flex_row_start()
            .flex_col()
            .children(warning(pairs).map(|text| note(text, p).text_color(p.warning)))
            .child(
                div()
                    .id("add-subtitles-pairs-area")
                    .flex_1()
                    .min_h_0()
                    .px(px(PADDING))
                    .role(accesskit::Role::List)
                    .aria_label(tr!("Videos and their subtitles"))
                    .child(list),
            )
            .into_any_element()
    }

    /// Video `index`: its name, the connector, and its subtitles with a picker.
    fn pair_row(
        &self,
        choice: &Choice,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let video = div()
            .flex_1()
            .min_w_0()
            .flex_row_start()
            .items_center()
            .gap(px(14.))
            .child(
                icon_and_name(icons::VIDEO, file_name(&choice.video), p)
                    .flex_none()
                    .max_w(px(380.)),
            )
            .child(connector(p));
        let row = div()
            .id(("add-subtitles-pair", index))
            .h(px(ROW_HEIGHT))
            .text_color(p.text_muted)
            .flex_row_start()
            .items_center()
            .ps(px(18.))
            .child(video)
            .child(self.subtitles_cell(choice, index, p, cx));
        spaced(row).into_any_element()
    }

    /// Video `index`'s subtitles, opening its picker on a click.
    fn subtitles_cell(
        &self,
        choice: &Choice,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let status = choice.chosen.iter().find_map(|subtitle| {
            let pair = (choice.video.clone(), subtitle.clone());
            let added = !choice.linked.contains(subtitle);
            self.pair_status(&pair, added, p, cx)
        });
        let cell = div()
            .id(("add-subtitles-pick", index))
            .tab_index(0)
            .key_context(focus::CONTROL)
            .flex_1()
            .min_w_0()
            .h_full()
            .flex_row_start()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .ps(px(16.))
            .rounded(px(metrics::RADIUS + 2.))
            .cursor_pointer()
            .hover(|style| style.bg(p.hover))
            .role(accesskit::Role::Button)
            .aria_label(SharedString::from(tr!(
                "Subtitles for {video}",
                video = file_name(&choice.video)
            )))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.open_picker(index, event.position, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if let Some(at) = focus::keyboard_anchor(event) {
                    this.open_picker(index, at, window, cx);
                }
            }))
            .child(chosen_label(choice, p))
            .children(status);
        focus::outline_ring(cell, p)
    }

    /// Opens video `index`'s picker at `position`.
    fn open_picker(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Body::Pairs(pairs) = &self.body else { return };
        let choice = &pairs.choices[index];
        let this = cx.entity().downgrade();
        let mut ranked = pairing::rank_for(&choice.video, &pairs.candidates);
        let elsewhere: Vec<PathBuf> =
            choice.chosen.iter().filter(|chosen| !ranked.contains(chosen)).cloned().collect();
        ranked.extend(elsewhere);
        let mut items: Vec<MenuItem> = ranked
            .into_iter()
            .map(|subtitle| {
                let (this, checked) = (this.clone(), choice.is_chosen(&subtitle));
                MenuItem::check(file_name(&subtitle), checked, move |_, cx| {
                    this.update(cx, |dialog, cx| dialog.toggle_pair(index, &subtitle, cx)).ok();
                })
            })
            .collect();
        if !items.is_empty() {
            items.push(MenuItem::Separator);
        }
        let clear = this.clone();
        items.push(MenuItem::action(tr!("No Match"), move |_, cx| {
            clear.update(cx, |dialog, cx| dialog.clear_pair(index, cx)).ok();
        }));
        items.push(MenuItem::action(tr!("Choose File…"), move |window, cx| {
            this.update(cx, |_, cx| Self::choose_pair_file(index, window, cx)).ok();
        }));
        let menu = ContextMenu::build(items, window, cx);
        let closed = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
            this.menu = None;
            this.focus_handle.focus(window, cx);
            cx.notify();
        });
        self.menu = Some((position, menu, closed));
        cx.notify();
    }

    /// Checks or unchecks `subtitle` for video `index`.
    fn toggle_pair(&mut self, index: usize, subtitle: &Path, cx: &mut Context<Self>) {
        if let Body::Pairs(pairs) = &mut self.body {
            pairs.choices[index].toggle(subtitle);
            self.changed(cx);
        }
    }

    /// "No Match": leaves video `index` without subtitles.
    fn clear_pair(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Body::Pairs(pairs) = &mut self.body {
            pairs.choices[index].chosen.clear();
            self.changed(cx);
        }
    }

    /// "Choose File…": asks for subtitle files anywhere and checks them for video `index`.
    fn choose_pair_file(index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some(tr!("Choose").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else { return };
            this.update(cx, |this, cx| this.add_pair_files(index, &paths, cx)).ok();
        })
        .detach();
    }

    /// Checks `paths` for video `index`.
    fn add_pair_files(&mut self, index: usize, paths: &[PathBuf], cx: &mut Context<Self>) {
        let Body::Pairs(pairs) = &mut self.body else { return };
        let choice = &mut pairs.choices[index];
        for path in paths {
            if !choice.is_chosen(path) {
                choice.toggle(path);
            }
        }
        self.changed(cx);
    }

    /// A pairing changed: rechecks the copies' names and redraws.
    pub(super) fn changed(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        self.check_conflicts(cx);
        cx.notify();
    }
}

/// What the list says above the pairs when the source folder fits none of the videos.
fn warning(pairs: &Pairs) -> Option<&'static str> {
    match (pairs.candidates.is_empty(), pairs.paired_any) {
        (true, _) => {
            Some(tr!("No subtitle files in this folder. Click the folder above to choose another."))
        }
        (false, false) => Some(tr!(
            "None of these subtitles seem to belong to these videos. Pick one for each video, \
             or choose another folder."
        )),
        (false, true) => None,
    }
}

/// The dashes between a video and its subtitles.
fn connector(p: &Palette) -> Div {
    div()
        .flex_1()
        .min_w(px(24.))
        .me(px(8.))
        .border_t_1()
        .border_dashed()
        .border_color(p.text_muted.opacity(CONNECTOR_OPACITY))
}

/// The first chosen subtitle and how many more, or "No match".
fn chosen_label(choice: &Choice, p: &Palette) -> Div {
    let Some(first) = choice.chosen.first() else {
        return icon_and_name(icons::SUBTITLE_FILE, tr!("No match").into(), p)
            .text_color(p.text_muted.opacity(0.6));
    };
    let more = choice.chosen.len() - 1;
    let more = (more > 0).then(|| {
        div()
            .flex_none()
            .text_size(text_px(metrics::TEXT_SMALL))
            .text_color(p.text_muted)
            .child(format!("+{more}"))
    });
    div()
        .min_w_0()
        .flex_row_start()
        .items_center()
        .gap(px(8.))
        .text_color(p.text_muted)
        .child(icon_and_name(icons::SUBTITLE_FILE, file_name(first), p))
        .children(more)
}
