//! The main window's content before the player screen exists (Stage 2): settings notices, a
//! drop zone with an Open button, what's opening or playing, and the opened list.

use gpui::{
    AnyElement, ClickEvent, Context, Div, ExternalPaths, SharedString, Stateful, div, prelude::*,
    px, svg,
};

use super::MainWindow;
use crate::assets::icons;
use crate::keymap::{self, actions::OpenFile};
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::toast::{Banner, Level, banner};

/// How many items of the opened list the window shows.
const LIST_ROWS: usize = 20;

impl MainWindow {
    pub(super) fn render_empty_state(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .min_h_full()
            .p(px(24.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .children(
                self.notices.iter().enumerate().map(|(i, notice)| notice_banner(i, notice, cx)),
            )
            .child(self.render_drop_zone(p, cx))
            .children(self.render_status(p, cx))
            .children(self.render_list(p))
            .into_any_element()
    }

    /// The opened list, the playing item marked, cut short when it is long.
    fn render_list(&self, p: &Palette) -> Option<Div> {
        if self.files.len() < 2 {
            return None;
        }
        let rows = self.files.iter().enumerate().take(LIST_ROWS).map(|(i, file)| {
            let playing = self.current == Some(i);
            let marker = if playing { "▶ " } else { "" };
            div()
                .text_size(px(13.))
                .text_color(if playing { p.text_strong } else { p.text_muted })
                .child(SharedString::from(format!("{marker}{}", file.display_name())))
        });
        let more = self.files.len().saturating_sub(LIST_ROWS);
        let more = (more > 0).then(|| {
            div().text_size(px(13.)).text_color(p.text_muted).child(format!("and {more} more"))
        });
        Some(div().flex().flex_col().items_center().gap(px(4.)).children(rows).children(more))
    }

    fn render_drop_zone(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let hover = p.hover;
        let accent_fg = p.accent_fg;

        div()
            .id("drop-zone")
            .w(px(520.))
            .max_w_full()
            .py(px(48.))
            .px(px(32.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .rounded(px(12.))
            .border_2()
            .border_dashed()
            .border_color(p.key_border)
            .drag_over::<ExternalPaths>(move |style, _, _, _| {
                style.border_color(accent_fg).bg(hover)
            })
            .child(svg().path(icons::FILE).size(px(48.)).text_color(p.icon))
            .child(
                div().text_size(px(20.)).text_color(p.text_strong).child("Drop files here to play"),
            )
            .child(div().text_size(px(14.)).text_color(p.text_muted).child("or"))
            .child(
                Button::new("open-file", "Open file…")
                    .primary()
                    .track_focus(&self.open_button)
                    .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                        Self::prompt_for_files(window, cx);
                    })),
            )
            .children(
                keymap::shortcut(&OpenFile, cx)
                    .map(|hint| div().text_size(px(12.)).text_color(p.text_muted).child(hint)),
            )
    }

    fn dismiss_notice(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.notices.len() {
            self.notices.remove(index);
        }
        cx.notify();
    }
}

/// The warning banner for notice `index`, which removes it when dismissed.
fn notice_banner(index: usize, notice: &SharedString, cx: &mut Context<MainWindow>) -> Banner {
    let this = cx.entity().downgrade();
    banner(("notice", index), Level::Warning, notice.clone()).on_dismiss(move |_, cx| {
        this.update(cx, |this, cx| this.dismiss_notice(index, cx)).ok();
    })
}
