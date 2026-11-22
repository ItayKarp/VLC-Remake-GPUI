//! What a multi-line text input does besides a single-line one's editing: Enter starts a line,
//! Up and Down move between lines (to the text's start or end from its first or last line), and
//! the wheel scrolls.

use std::time::Instant;

use gpui::{Context, Div, InteractiveElement as _, ScrollEvent, Stateful, Window};

use super::TextInput;
use super::keys::{Down, Newline, SelectDown, SelectUp, Up};

/// Which way Up and Down move.
#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
}

impl TextInput {
    /// Wires a multi-line field's own actions and its wheel.
    pub(super) fn multi_line_actions(
        field: Stateful<Div>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        field
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_scroll(cx.listener(Self::on_scroll))
    }

    /// Replaces the selection with a line break.
    pub(super) fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        let changed = self.state.replace(None, "\n", Instant::now());
        self.edited(changed, window, cx);
    }

    /// Moves the caret up a line.
    pub(super) fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.line_step(Direction::Up);
        self.state.move_to(target);
        self.moved(window, cx);
    }

    /// Moves the caret down a line.
    pub(super) fn down(&mut self, _: &Down, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.line_step(Direction::Down);
        self.state.move_to(target);
        self.moved(window, cx);
    }

    /// Extends the selection up a line.
    pub(super) fn select_up(&mut self, _: &SelectUp, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.line_step(Direction::Up);
        self.state.select_to(target);
        self.moved(window, cx);
    }

    /// Extends the selection down a line.
    pub(super) fn select_down(
        &mut self,
        _: &SelectDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let target = self.line_step(Direction::Down);
        self.state.select_to(target);
        self.moved(window, cx);
    }

    /// Scrolls the lines, without moving the caret.
    pub(super) fn on_scroll(
        &mut self,
        event: &ScrollEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(lines) = self.last_layout.as_ref() else { return };
        let delta = event.delta.pixel_delta(lines.line_height());
        self.scroll -= delta;
        // The next paint keeps the scroll within the text.
        cx.notify();
        cx.stop_propagation();
    }

    /// Where the caret goes a line up or down, at the same distance from the left: the text's
    /// start from its first line, its end from its last.
    fn line_step(&self, direction: Direction) -> usize {
        let cursor = self.state.cursor();
        let Some(lines) = self.last_layout.as_ref().filter(|_| !self.state.text().is_empty())
        else {
            return cursor;
        };
        let line = lines.line_of(cursor);
        let x = lines.position(cursor).x;
        match direction {
            Direction::Up if line == 0 => 0,
            Direction::Up => lines.index_on_line(line - 1, x),
            Direction::Down if line + 1 >= lines.count() => self.state.text().len(),
            Direction::Down => lines.index_on_line(line + 1, x),
        }
    }
}
