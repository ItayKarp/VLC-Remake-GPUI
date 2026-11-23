//! The Chapters tab (S8.2.1, `controls-chapters.png`): a row per chapter of the loaded media, its
//! number in the accent, its title and where it starts. Clicking a row goes there.
//!
//! The playing chapter is marked as the Queue tab marks the playing entry, with its title in the
//! accent and an accent bar at its left edge (the design has no highlight). The arrows, Home and
//! End move a cursor over the rows, drawn filled, and Enter goes to the chapter under it.

use gpui::{
    ClickEvent, Context, Div, Hsla, KeyDownEvent, ScrollHandle, SharedString, Stateful, Window,
    accesskit, div, prelude::*, px,
};

use super::{MainWindow, note};
use crate::main_window::control_bar::clock::{self, tabular_digits};
use crate::main_window::seeking::chapters::{self, Listed};
use crate::theme::Palette;

/// A row's height, text size, number column width and side padding, as designed.
const ROW_HEIGHT: f32 = 44.;
const TEXT: f32 = 14.;
const NUMBER_WIDTH: f32 = 44.;
const PADDING: f32 = 24.;
/// The space above the first row.
const TOP: f32 = 16.;
/// The playing chapter's bar.
const BAR_WIDTH: f32 = 3.;

/// The tab's own state: the keyboard's row and the scrolling.
#[derive(Default)]
pub(super) struct ChapterList {
    cursor: Option<usize>,
    scroll: ScrollHandle,
}

impl MainWindow {
    /// The rows, or a note when the media has none.
    pub(super) fn render_chapters_tab(
        &self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let listed = self.chapters(cx);
        if listed.is_empty() {
            return note("This media has no chapters.", p);
        }
        let position = self.loaded_player(cx).map(|player| player.read(cx).position());
        let current = position.and_then(|position| chapters::current(&listed, position));
        let focused = self.queue_panel.focus.is_focused(window);
        let cursor = self.queue_panel.chapters.cursor.filter(|_| focused);
        let rows = listed.into_iter().enumerate().map(|(index, chapter)| {
            let look = Look { current: current == Some(index), cursor: cursor == Some(index) };
            chapter_row(index, chapter, look, p, cx)
        });
        let list = div()
            .id("chapter-rows")
            .size_full()
            .pt(px(TOP))
            .overflow_y_scroll()
            .track_scroll(&self.queue_panel.chapters.scroll)
            .role(accesskit::Role::List)
            .aria_label("Chapters")
            .children(rows);
        div().flex_1().min_h_0().child(list)
    }

    /// The tab's keys: the arrows, Home and End move the cursor and Enter goes to its chapter.
    /// Returns whether the key was used.
    pub(super) fn chapters_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let listed = self.chapters(cx);
        let Some(last) = listed.len().checked_sub(1) else { return false };
        let at = self.queue_panel.chapters.cursor.map(|cursor| cursor.min(last));
        let moved = match event.keystroke.key.as_str() {
            "up" => at.map_or(last, |at| at.saturating_sub(1)),
            "down" => at.map_or(0, |at| (at + 1).min(last)),
            "home" => 0,
            "end" => last,
            "enter" => {
                let Some(at) = at else { return false };
                self.seek_to_chapter(&listed[at], cx);
                return true;
            }
            _ => return false,
        };
        let state = &mut self.queue_panel.chapters;
        state.cursor = Some(moved);
        state.scroll.scroll_to_item(moved);
        cx.notify();
        true
    }

    /// Goes to the chapter of the clicked row, which takes the cursor.
    fn click_chapter(&mut self, index: usize, chapter: &Listed, cx: &mut Context<Self>) {
        self.queue_panel.chapters.cursor = Some(index);
        self.seek_to_chapter(chapter, cx);
        cx.notify();
    }
}

/// How a row is marked.
#[derive(Clone, Copy)]
struct Look {
    /// It is the playing chapter.
    current: bool,
    /// The keyboard is on it.
    cursor: bool,
}

/// One chapter's row: number, title and start.
fn chapter_row(
    index: usize,
    chapter: Listed,
    look: Look,
    p: &Palette,
    cx: &mut Context<MainWindow>,
) -> Stateful<Div> {
    let spoken = format!("{}. {}, {}", chapter.number, chapter.title, clock::format(chapter.start));
    let title_color = if look.current { p.accent_fg } else { p.text };
    let row = div()
        .id(("chapter-row", index))
        .relative()
        .w_full()
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .px(px(PADDING))
        .text_size(px(TEXT))
        .cursor_pointer()
        .role(accesskit::Role::ListItem)
        .aria_label(SharedString::from(spoken))
        .when(look.cursor, |row| row.bg(p.selected))
        .when(!look.cursor, |row| row.hover(|style| style.bg(p.hover)))
        .when(look.current, |row| {
            row.child(div().absolute().left_0().top_0().bottom_0().w(px(BAR_WIDTH)).bg(p.accent))
        })
        .children(cells(&chapter, title_color, p));
    row.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        this.click_chapter(index, &chapter, cx);
    }))
}

/// A row's number, title (in `title_color`) and start.
fn cells(chapter: &Listed, title_color: Hsla, p: &Palette) -> [Div; 3] {
    [
        div()
            .flex_none()
            .w(px(NUMBER_WIDTH))
            .text_color(p.accent_fg)
            .child(chapter.number.to_string()),
        div().flex_1().min_w_0().truncate().text_color(title_color).child(chapter.title.clone()),
        div()
            .flex_none()
            .text_color(p.text_muted)
            .font_features(tabular_digits())
            .child(clock::format(chapter.start)),
    ]
}
