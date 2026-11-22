//! Browsing the listing (S4.6.5): a single click selects an item, a double-click or Enter opens
//! it, and the arrow keys, Home, End and Page Up and Down move the selection. Opening a card plays
//! it with the folder's other cards after it as the queue (folder playback, S3); opening a row
//! does what it says (goes to its place, plays, adds a folder).
//!
//! The context-menu key and Shift+F10 open the keyboard's item's menu (S4.8.4).
//!
//! The listing is one tab stop, drawn by `uniform_list`, which builds only the rows (or the lines
//! of tiles, [`grid`](super::grid)) on screen; the media on screen is what the thumbnailer makes
//! first (S4.4.4). In a grid, Left and Right step through a line and Up and Down jump one.

use std::ops::Range;

use gpui::{
    AnyElement, ClickEvent, Context, Div, KeyDownEvent, ScrollStrategy, Stateful, Window, div,
    prelude::*, px, uniform_list,
};

use super::MainWindow;
use super::grid::{COLUMNS, LINE_HEIGHT};
use super::listing::{Body, Item};
use super::row::{ROW_HEIGHT, ROW_TEXT};
use super::view_menu::line_of;
use crate::app::media::Media;
use crate::app::request::Placement;
use crate::db::media::MediaId;
use crate::keymap::actions::ShowContextMenu;
use crate::library::thumbnails;
use crate::main_window::opening::Target;
use crate::navigation::history::Step;
use crate::navigation::route::{Route, Section};
use crate::navigation::view::View;
use crate::opening::Resolved;
use crate::theme::{self, Palette};
use crate::ui::button::Button;
use crate::ui::focus::{self, Axis, Layout, Move};
use crate::ui::list::row_count;

/// Air between the header's divider and the first row.
const LIST_TOP: f32 = 15.;

impl MainWindow {
    /// The rows under the header, or why there are none.
    pub(super) fn render_listing(&self, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let body = div()
            .id("library-listing")
            .flex_1()
            .min_h_0()
            .track_focus(&self.library_panel.list_focus)
            // A control: the player's bindings of the arrows and the page keys leave it alone.
            .key_context(focus::context(&[focus::CONTROL]))
            .on_key_down(cx.listener(Self::listing_key))
            // The context-menu key and Shift+F10 open the keyboard's item's menu (S4.8.4).
            .on_action(cx.listener(|this, _: &ShowContextMenu, window, cx| {
                this.open_item_menu_from_keyboard(window, cx);
            }));
        if *self.library_panel.history.current() == Route::Home {
            return body.pt(px(LIST_TOP)).child(self.render_home(p, cx));
        }
        let view = self.library_view(cx);
        let len = match self.listing(cx) {
            Body::Items(items) if items.len() == 0 => return body.child(self.render_empty(p, cx)),
            Body::Items(items) => items.len(),
            Body::Reading => return body,
            Body::Gone(message) => return body.child(self.render_gone(message, p, cx)),
        };
        let lines = match view {
            View::List => len,
            View::Grid => row_count(len, COLUMNS),
        };
        let list = uniform_list(
            "library-rows",
            lines,
            cx.processor(move |this, range: Range<usize>, window, cx| {
                let p = theme::palette(window, cx);
                match view {
                    View::List => this.render_items(range, &p, cx),
                    View::Grid => this.render_lines(range, &p, cx),
                }
            }),
        )
        .track_scroll(&self.library_panel.scroll)
        .size_full()
        .px(px(super::INSET));
        body.pt(px(LIST_TOP)).child(list)
    }

    /// The items in `range`, and the thumbnailer told what is on screen.
    fn render_items(
        &mut self,
        range: Range<usize>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Body::Items(items) = self.listing(cx) else { return Vec::new() };
        let mut on_screen: Vec<MediaId> = Vec::new();
        let mut elements = Vec::new();
        for index in range {
            let element = match items.get(index) {
                Some(Item::Row(row)) => {
                    let selected = self.library_panel.selection.contains(&row.key());
                    row.render(index, selected, p, cx).into_any_element()
                }
                Some(Item::Card(card)) => {
                    on_screen.extend(card.media);
                    self.render_card(index, card, p, cx).into_any_element()
                }
                None => continue,
            };
            elements.push(element);
        }
        self.show_thumbnails(on_screen, cx);
        elements
    }

    /// Tells the thumbnailer which media are on screen, when that changed.
    pub(super) fn show_thumbnails(&mut self, on_screen: Vec<MediaId>, cx: &mut Context<Self>) {
        if on_screen == self.library_panel.on_screen {
            return;
        }
        self.library_panel.on_screen.clone_from(&on_screen);
        // Not while drawing: the service's change redraws the window.
        cx.defer(move |cx| thumbnails::show(&on_screen, cx));
    }

    /// A click on item `index`: selects only it, adds or removes it with Ctrl/Cmd, or selects
    /// the range to it with Shift. A plain double-click opens it.
    pub(super) fn click_item(
        &mut self,
        index: usize,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.library_panel.list_focus.focus(window, cx);
        let Body::Items(items) = self.listing(cx) else { return };
        let keys = items.keys();
        let Some(key) = keys.get(index).cloned() else { return };
        let modifiers = event.modifiers();
        let selection = &mut self.library_panel.selection;
        if modifiers.shift {
            selection.extend_to(&keys, index);
        } else if modifiers.secondary(cx) {
            selection.toggle(key);
        } else {
            selection.select(key);
            if event.click_count() >= 2 {
                self.open_item(index, window, cx);
            }
        }
        cx.notify();
    }

    /// Where the keyboard's item is in the listing, if it is listed.
    pub(super) fn cursor_index(&self, cx: &gpui::App) -> Option<usize> {
        let Body::Items(items) = self.listing(cx) else { return None };
        self.library_panel.selection.cursor(&items.keys())
    }

    /// Opens item `index`: plays a card with the other cards queued, or follows a row.
    pub(super) fn open_item(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Body::Items(items) = self.listing(cx) else { return };
        match items.get(index) {
            Some(Item::Card(card)) => {
                // Home's media are unrelated: one plays alone.
                if self.library_panel.history.current().section() == Section::Home {
                    let media = vec![Media::File(card.path.clone())];
                    self.play_cards(media, 0, window, cx);
                    return;
                }
                let start = items.card_index(index).unwrap_or_default();
                self.play_cards(items.card_media(), start, window, cx);
            }
            Some(Item::Row(row)) => {
                let target = row.target().clone();
                self.follow(target, window, cx);
            }
            None => {}
        }
    }

    /// Replaces the queue with `media` and plays item `start`.
    pub(super) fn play_cards(
        &mut self,
        media: Vec<Media>,
        start: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_opening(cx);
        let mut resolved = Resolved { media, start, ..Resolved::default() };
        self.play_resolved(&mut resolved, Target::Placed(Placement::Replace), window, cx);
    }

    /// Enter opens the keyboard's item, Ctrl/Cmd+A selects everything, and the arrows, Home,
    /// End and the page keys move the selection (Shift extends it).
    fn listing_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Body::Items(items) = self.listing(cx) else { return };
        let keys = items.keys();
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let view = self.library_view(cx);
        let layout = self.library_panel.key_layout(view);
        let selection = &mut self.library_panel.selection;
        if keystroke.key == "enter" && !modifiers.modified() {
            if let Some(index) = selection.cursor(&keys) {
                cx.stop_propagation();
                self.open_item(index, window, cx);
            }
            return;
        }
        if let Some(step) = history_step(event) {
            cx.stop_propagation();
            self.step_library(step, cx);
            return;
        }
        if keystroke.key == "a" && modifiers.secondary(cx) && !modifiers.shift {
            selection.select_all(&keys);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let Some((movement, extend)) = movement(event, view) else { return };
        if let Some(index) = selection.step(&keys, movement, layout, extend) {
            cx.stop_propagation();
            self.scroll_listing_to(index, ScrollStrategy::Nearest, cx);
            cx.notify();
        }
    }

    /// Scrolls item `index` into view, by `strategy` in the virtual list; Home's page scrolls
    /// just far enough.
    pub(super) fn scroll_listing_to(&self, index: usize, strategy: ScrollStrategy, cx: &gpui::App) {
        if let Some(child) = self.home_child(index) {
            self.library_panel.home_scroll.scroll_to_item(child);
            return;
        }
        let line = line_of(index, self.library_view(cx));
        self.library_panel.scroll.scroll_to_item(line, strategy);
    }

    fn render_gone(&self, message: &'static str, p: &Palette, cx: &mut Context<Self>) -> Div {
        let back = Button::new("library-gone-back", "Back")
            .disabled(!self.can_step_library(Step::Back, cx))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.step_library(Step::Back, cx);
            }));
        note(message, p).flex().flex_col().items_start().gap(px(12.)).child(back)
    }
}

impl super::LibraryPanel {
    /// How the keyboard moves through the items shown as `view`: a page key skips what fits on
    /// screen.
    fn key_layout(&self, view: View) -> Layout {
        let shown = f32::from(self.scroll.0.borrow().base_handle.bounds().size.height);
        match view {
            View::List => Layout { page: ((shown / ROW_HEIGHT) as usize).max(1), ..Layout::LIST },
            View::Grid => Layout::grid(COLUMNS, (shown / LINE_HEIGHT) as usize),
        }
    }
}

/// The library's Alt+Left and Alt+Right skip controls, so the listing goes back and forward
/// itself.
fn history_step(event: &KeyDownEvent) -> Option<Step> {
    let modifiers = event.keystroke.modifiers;
    if !modifiers.alt || modifiers.shift || modifiers.control || modifiers.platform {
        return None;
    }
    match event.keystroke.key.as_str() {
        "left" => Some(Step::Back),
        "right" => Some(Step::Forward),
        _ => None,
    }
}

/// Where a key moves the selection among items shown as `view`, and whether it extends it
/// (Shift).
fn movement(event: &KeyDownEvent, view: View) -> Option<(Move, bool)> {
    let modifiers = event.keystroke.modifiers;
    let shift_only = modifiers.shift && !(modifiers.control || modifiers.alt || modifiers.platform);
    let axis = match view {
        View::List => Axis::Vertical,
        View::Grid => Axis::Grid,
    };
    if shift_only {
        Move::from_key(&event.keystroke.key, axis).map(|movement| (movement, true))
    } else {
        focus::group_key(event, axis).map(|movement| (movement, false))
    }
}

/// A muted line of text in the panel.
fn note(text: &'static str, p: &Palette) -> Div {
    div().p(px(super::INSET)).text_size(px(ROW_TEXT)).text_color(p.text_muted).child(text)
}
