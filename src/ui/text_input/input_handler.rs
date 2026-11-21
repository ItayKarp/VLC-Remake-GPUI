//! The platform input handler: IME composition and accessibility queries.
//!
//! The platform speaks UTF-16 offsets; they're converted to UTF-8 here, at the edge.

use std::ops::Range;
use std::time::Instant;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, point};

use super::TextInput;
use super::utf16;

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.state.range_from_utf16(&range_utf16);
        actual_range.replace(self.state.range_to_utf16(&range));
        Some(self.state.text()[range].to_owned())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.state.range_to_utf16(&self.state.selection()),
            reversed: self.state.is_reversed(),
        })
    }

    fn set_selected_text_range(
        &mut self,
        range_utf16: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = self.state.range_from_utf16(&range_utf16);
        self.state.select_range(range);
        self.moved(window, cx);
    }

    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.state.offset_to_utf16(self.state.text().len()))
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.state.marked().map(|range| self.state.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.state.unmark();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16.map(|r| self.state.range_from_utf16(&r));
        let changed = self.state.replace(range, new_text, Instant::now());
        self.edited(changed, window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let range = range_utf16.map(|r| self.state.range_from_utf16(&r));
        // The selection inside the new text arrives in UTF-16 too.
        let selected = new_selected_range_utf16.map(|sel| utf16::range_from_utf16(new_text, &sel));
        self.state.replace_and_mark(range, new_text, selected);
        self.edited(true, window, cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let line = self.last_layout.as_ref()?;
        let range = self.state.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start) - self.scroll_x, bounds.top()),
            point(bounds.left() + line.x_for_index(range.end) - self.scroll_x, bounds.bottom()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let local = self.last_bounds?.localize(&point)?;
        let line = self.last_layout.as_ref()?;
        let index = line.closest_index_for_x(local.x + self.scroll_x);
        Some(self.state.offset_to_utf16(index))
    }
}
