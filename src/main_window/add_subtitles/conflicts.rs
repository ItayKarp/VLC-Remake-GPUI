//! What a row shows at its end: why it couldn't be saved, or, with "Copy next to video" on,
//! that its copy's name is already taken next to the video, with Overwrite, Keep both and Skip
//! to choose from (S6.4 edge case). Names are checked on the disk in the background whenever
//! the checkbox or the checked subtitles change.

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, DismissEvent, Div, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString, Stateful, accesskit, div, prelude::*, px,
};

use super::model::Pair;
use super::{AddSubtitlesDialog, Body};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::subtitle_links::copy::{self, OnConflict};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::focus;
use crate::ui::icon::icon;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::metrics;

/// The "Name taken: …" chip for `pair`, a tab stop that opens the choices.
fn conflict_chip(pair: &Pair, label: SharedString, p: &Palette) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("conflict-{}", pair.1.display())))
        .tab_index(0)
        .key_context(focus::CONTROL)
        .role(accesskit::Role::Button)
        .aria_label(label.clone())
        .flex_none()
        .flex_row_start()
        .items_center()
        .gap(px(4.))
        .px(px(8.))
        .py(px(2.))
        .rounded(px(metrics::RADIUS))
        .bg(p.control)
        .hover(|style| style.bg(p.button_hover))
        .text_size(text_px(metrics::TEXT_SMALL))
        .text_color(p.warning)
        .child(label)
        .child(icon(icons::CHEVRON_DOWN, 14., p.icon))
}

/// The widest a row's error gets before it is cut short.
const ERROR_WIDTH: f32 = 320.;

impl AddSubtitlesDialog {
    /// `pair`'s error, or the choice for its taken name when it is `added` and copied.
    pub(super) fn pair_status(
        &self,
        pair: &Pair,
        added: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if let Some(error) = self.errors.get(pair) {
            return Some(error_label(error, p).into_any_element());
        }
        let choice = self.copying.on.then(|| self.copying.conflicts.get(pair)).flatten()?;
        if !added {
            return None;
        }
        let label =
            SharedString::from(tr!("Name taken: {choice}", choice = conflict_label(*choice)));
        let pair = pair.clone();
        let keyboard_pair = pair.clone();
        let button = conflict_chip(&pair, label, p)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_conflict_menu(&pair, event.position, window, cx);
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                if let Some(at) = focus::keyboard_anchor(event) {
                    this.open_conflict_menu(&keyboard_pair, at, window, cx);
                }
            }));
        Some(focus::outline_ring(button, p).into_any_element())
    }

    /// The menu of what to do with `pair`'s taken name.
    fn open_conflict_menu(
        &mut self,
        pair: &Pair,
        position: Point<Pixels>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        let current = self.copying.conflicts.get(pair).copied().unwrap_or_default();
        let this = cx.entity().downgrade();
        let items = [OnConflict::Overwrite, OnConflict::KeepBoth, OnConflict::Skip]
            .into_iter()
            .map(|choice| {
                let (this, pair) = (this.clone(), pair.clone());
                MenuItem::radio(conflict_label(choice), choice == current, move |_, cx| {
                    this.update(cx, |dialog, cx| dialog.settle(pair.clone(), choice, cx)).ok();
                })
            })
            .collect();
        let menu = ContextMenu::build(items, window, cx);
        let closed = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
            this.menu = None;
            this.focus_handle.focus(window, cx);
            cx.notify();
        });
        self.menu = Some((position, menu, closed));
        cx.notify();
    }

    /// Checks the names of the copies Save would make that weren't checked yet.
    pub(super) fn check_conflicts(&mut self, cx: &mut Context<Self>) {
        if !self.copying.on {
            return;
        }
        let unchecked: Vec<Pair> = self
            .additions()
            .into_iter()
            .filter(|pair| !self.copying.checked.contains(pair))
            .collect();
        if unchecked.is_empty() {
            return;
        }
        let check = cx.background_spawn(async move {
            unchecked
                .into_iter()
                .map(|pair| {
                    let taken = copy::conflicts(&pair.0, &pair.1);
                    (pair, taken)
                })
                .collect::<Vec<_>>()
        });
        self.conflict_check = Some(cx.spawn(async move |this, cx| {
            let checked = check.await;
            this.update(cx, |this, cx| this.conflicts_checked(checked, cx)).ok();
        }));
    }

    /// Settles `pair`'s taken name with `choice`.
    fn settle(&mut self, pair: Pair, choice: OnConflict, cx: &mut Context<Self>) {
        self.copying.conflicts.insert(pair, choice);
        cx.notify();
    }

    /// Records which of the `checked` copies' names are taken: Keep both until the user says.
    fn conflicts_checked(&mut self, checked: Vec<(Pair, bool)>, cx: &mut Context<Self>) {
        for (pair, taken) in checked {
            if taken {
                self.copying.conflicts.entry(pair.clone()).or_default();
            }
            self.copying.checked.insert(pair);
        }
        cx.notify();
    }

    /// The subtitles Save would link that aren't linked yet, with their videos.
    pub(super) fn additions(&self) -> Vec<Pair> {
        let choices: &[_] = match &self.body {
            Body::Files(files) => std::slice::from_ref(&files.choice),
            Body::Pairs(pairs) => &pairs.choices,
            _ => &[],
        };
        choices
            .iter()
            .flat_map(|choice| {
                choice.additions().map(|subtitle| (choice.video.clone(), subtitle.clone()))
            })
            .collect()
    }
}

/// How the conflict menu and button name `choice`.
fn conflict_label(choice: OnConflict) -> &'static str {
    match choice {
        OnConflict::Overwrite => tr!("Overwrite"),
        OnConflict::KeepBoth => tr!("Keep both"),
        OnConflict::Skip => tr!("Skip"),
    }
}

/// A row's error: an icon and why, cut short when long.
fn error_label(error: &str, p: &Palette) -> Div {
    let text = div().min_w_0().max_w(px(ERROR_WIDTH)).truncate().child(error.to_owned());
    div()
        .flex_row_start()
        .items_center()
        .gap(px(6.))
        .text_color(p.danger)
        .text_size(text_px(metrics::TEXT_SMALL))
        .child(icon(icons::INFO, 16., p.danger))
        .child(text)
}
