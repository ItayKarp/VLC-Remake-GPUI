//! The Bookmarks tab (S8.3.1, `controls-bookmarks.png`): "N Bookmarks" with "−" and "+", then a
//! row per bookmark of the playing media, its name and its place (`00:00:42.023`).
//!
//! A click goes to the bookmark and a double-click renames it in place ([`rename`]). The rows are
//! virtualized, so a thousand bookmarks draw only what's on screen. The arrows, Home and End move
//! a cursor over the rows, drawn filled; Enter goes to its bookmark, F2 renames it and Delete (or
//! Backspace, or "−") removes it. "+" bookmarks where playback is ([`store`] keeps them).
//! A bookmark past the end of the media is greyed.

mod naming;
mod rename;
mod store;

use std::ops::Range;

use gpui::{
    ClickEvent, Context, Div, KeyDownEvent, ScrollStrategy, SharedString, Stateful,
    UniformListScrollHandle, Window, accesskit, div, prelude::*, px, uniform_list,
};

pub(in crate::main_window) use self::naming::media_title;
use self::naming::timestamp;
use self::rename::Rename;
pub(super) use self::store::Bookmarks;
use self::store::{Bookmarkable, past_the_end};
use super::{MainWindow, QueuePanel, note};
use crate::assets::icons;
use crate::db::bookmarks::Bookmark;
use crate::i18n::DirectionalStyle;
use crate::keymap::{self, actions as act};
use crate::main_window::control_bar::clock::tabular_digits;
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::button::IconButton;

/// The header's height, its padding and its buttons' sizes, as the Queue tab's.
const HEADER_HEIGHT: f32 = 52.;
const HEADER_LEFT: f32 = 28.;
const HEADER_RIGHT: f32 = 16.;
const BUTTON: f32 = 32.;
const ICON: f32 = 20.;
const BUTTON_GAP: f32 = 4.;
/// A row's height, text size, side padding and the gap between the name and the place.
const ROW_HEIGHT: f32 = 44.;
const TEXT: f32 = 14.;
const PADDING: f32 = 24.;
const GAP: f32 = 12.;

/// The tab's own state: the keyboard's row, the scrolling and a rename in progress.
#[derive(Default)]
pub(super) struct BookmarkList {
    cursor: Option<usize>,
    scroll: UniformListScrollHandle,
    rename: Option<Rename>,
}

impl QueuePanel {
    /// Puts the Bookmarks tab's cursor on row `index` and scrolls to it, for a bookmark just
    /// added or picked.
    pub(in crate::main_window) fn point_at_bookmark(&mut self, index: usize) {
        self.bookmarks.cursor = Some(index);
        self.bookmarks.scroll.scroll_to_item(index, ScrollStrategy::Nearest);
    }

    /// Whether a bookmark is being renamed, so the panel leaves Escape to it.
    pub(super) fn is_renaming_bookmark(&self) -> bool {
        self.bookmarks.rename.is_some()
    }
}

impl MainWindow {
    /// The header and the rows, or a note when the media takes no bookmarks.
    pub(super) fn render_bookmarks_tab(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let why_none = match self.bookmarkable(cx) {
            Bookmarkable::NothingLoaded => Some(tr!("Open media to bookmark places in it.")),
            Bookmarkable::CantSeek => {
                Some(tr!("Live streams and media that can't seek take no bookmarks."))
            }
            Bookmarkable::Unstorable => {
                Some(tr!("This media can't be read, so it takes no bookmarks."))
            }
            Bookmarkable::Reading | Bookmarkable::Yes => None,
        };
        if let Some(why) = why_none {
            return note(why, p);
        }
        let len = self.bookmark_list(cx).len();
        let body = if len == 0 {
            note(tr!("No bookmarks yet. + marks where playback is."), p)
        } else {
            div().flex_1().min_h_0().child(self.render_bookmark_rows(len, cx))
        };
        div()
            .flex_1()
            .min_h_0()
            .flex_row_start()
            .flex_col()
            .child(self.render_bookmarks_header(len, p, cx))
            .child(body)
    }

    /// "N Bookmarks", then "−" (the cursor's bookmark) and "+".
    fn render_bookmarks_header(&self, len: usize, p: &Palette, cx: &mut Context<Self>) -> Div {
        let count = trn!("{count} Bookmark", "{count} Bookmarks", len);
        let cursor = self.bookmark_cursor(cx);
        let remove = IconButton::new("bookmark-remove", icons::REMOVE, tr!("Remove bookmark"))
            .size(BUTTON, ICON)
            .disabled(cursor.is_none())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(index) = cursor {
                    this.remove_bookmark(index, cx);
                }
            }));
        let add = IconButton::new("bookmark-add", icons::ADD, tr!("Add bookmark"))
            .size(BUTTON, ICON)
            .disabled(self.bookmarkable(cx) != Bookmarkable::Yes)
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.add_bookmark(cx)));
        let add = match keymap::shortcut(&act::AddBookmark, cx) {
            Some(shortcut) => add.shortcut(shortcut),
            None => add,
        };
        div()
            .flex_none()
            .h(px(HEADER_HEIGHT))
            .flex_row_start()
            .items_center()
            .justify_between()
            .ps(px(HEADER_LEFT))
            .pe(px(HEADER_RIGHT))
            .text_size(text_px(TEXT))
            .text_color(p.text_muted)
            .child(div().min_w_0().truncate().child(count))
            .child(div().flex_row_start().flex_none().gap(px(BUTTON_GAP)).child(remove).child(add))
    }

    fn render_bookmark_rows(&self, len: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let list = uniform_list(
            "bookmark-rows",
            len,
            cx.processor(|this, range: Range<usize>, window, cx| {
                let p = theme::palette(window, cx);
                this.render_bookmark_range(range, &p, window, cx)
            }),
        )
        .track_scroll(&self.queue_panel.bookmarks.scroll)
        .size_full();
        div()
            .id("bookmark-rows-area")
            .size_full()
            .role(accesskit::Role::List)
            .aria_label(tr!("Bookmarks"))
            .child(list)
    }

    fn render_bookmark_range(
        &self,
        range: Range<usize>,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<Stateful<Div>> {
        let duration = self.loaded_player(cx).and_then(|player| player.read(cx).duration());
        let focused = self.queue_panel.focus.is_focused(window);
        let cursor = self.bookmark_cursor(cx).filter(|_| focused);
        let list = self.bookmark_list(cx);
        range
            .filter_map(|index| {
                let bookmark = list.get(index)?;
                let look = Look {
                    index,
                    cursor: cursor == Some(index),
                    past_the_end: past_the_end(bookmark.position, duration),
                };
                Some(self.bookmark_row(bookmark, look, p, cx))
            })
            .collect()
    }

    fn bookmark_row(
        &self,
        bookmark: &Bookmark,
        look: Look,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let index = look.index;
        let place = timestamp(bookmark.position);
        let spoken = format!("{}, {place}", bookmark.name);
        let (name_color, place_color) =
            if look.past_the_end { (p.text_muted, p.text_muted) } else { (p.text, p.text_muted) };
        let name = match self.renaming(bookmark) {
            Some(input) => rename::field(input, p),
            None => div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(name_color)
                .child(bookmark.name.clone()),
        };
        div()
            .id(("bookmark-row", index))
            .w_full()
            .h(px(ROW_HEIGHT))
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .px(px(PADDING))
            .text_size(text_px(TEXT))
            .cursor_pointer()
            .role(accesskit::Role::ListItem)
            .aria_label(SharedString::from(spoken))
            .when(look.cursor, |row| row.bg(p.selected))
            .when(!look.cursor, |row| row.hover(|style| style.bg(p.hover)))
            .child(name)
            .child(
                div()
                    .flex_none()
                    .text_color(place_color)
                    .font_features(tabular_digits())
                    .child(place),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                this.click_bookmark(index, event.click_count(), window, cx);
            }))
    }

    /// A click goes to the bookmark, a double-click renames it; either way it takes the cursor.
    fn click_bookmark(
        &mut self,
        index: usize,
        clicks: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.queue_panel.bookmarks.cursor = Some(index);
        if clicks >= 2 {
            self.start_bookmark_rename(index, window, cx);
        } else {
            self.go_to_bookmark(index, cx);
        }
        cx.notify();
    }

    /// The cursor's row, kept within the list.
    fn bookmark_cursor(&self, cx: &gpui::App) -> Option<usize> {
        let last = self.bookmark_list(cx).len().checked_sub(1)?;
        self.queue_panel.bookmarks.cursor.map(|cursor| cursor.min(last))
    }

    /// The tab's keys. Returns whether the key was used; while a bookmark is renamed, the
    /// field has them all but Escape, which cancels it.
    pub(super) fn bookmarks_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.queue_panel.is_renaming_bookmark() {
            if event.keystroke.key == "escape" {
                self.cancel_bookmark_rename(window, cx);
                return true;
            }
            return false;
        }
        let Some(last) = self.bookmark_list(cx).len().checked_sub(1) else { return false };
        let at = self.bookmark_cursor(cx);
        let moved = match event.keystroke.key.as_str() {
            "up" => at.map_or(last, |at| at.saturating_sub(1)),
            "down" => at.map_or(0, |at| (at + 1).min(last)),
            "home" => 0,
            "end" => last,
            key => return at.is_some_and(|at| self.bookmark_command(key, at, window, cx)),
        };
        self.queue_panel.point_at_bookmark(moved);
        cx.notify();
        true
    }

    /// Enter, F2 and Delete on the cursor's bookmark at `at`. Returns whether `key` is one.
    fn bookmark_command(
        &mut self,
        key: &str,
        at: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        match key {
            "enter" => self.go_to_bookmark(at, cx),
            "f2" => self.start_bookmark_rename(at, window, cx),
            "delete" | "backspace" => self.remove_bookmark(at, cx),
            _ => return false,
        }
        true
    }
}

/// Which row it is and how it is marked.
#[derive(Clone, Copy)]
struct Look {
    /// Its place in the list.
    index: usize,
    /// The keyboard is on it.
    cursor: bool,
    /// It lies at or past the media's end.
    past_the_end: bool,
}
