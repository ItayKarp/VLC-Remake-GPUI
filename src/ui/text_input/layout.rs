//! The shaped text of an input, one [`ShapedLine`] per line, and the geometry the caret, the
//! selection, the mouse and the IME ask of it. A single-line input has one line; a multi-line
//! one has a line per `\n`. Positions are relative to the text's top-left corner, before any
//! scrolling.

use std::ops::Range;

use gpui::{
    Bounds, Pixels, Point, ShapedLine, SharedString, TextRun, UnderlineStyle, Window, point, px,
};

/// Thickness of the underline under an IME composition.
const COMPOSITION_UNDERLINE: Pixels = px(1.);

/// How far a selection that runs on past a line's end reaches beyond its last character, so a
/// selected line break shows.
const LINE_BREAK_WIDTH: Pixels = px(4.);

/// One shaped line and where it starts in the text.
struct Line {
    start: usize,
    shaped: ShapedLine,
}

impl Line {
    /// Where the line ends in the text, before its line break.
    fn end(&self) -> usize {
        self.start + self.shaped.len
    }
}

/// The shaped lines of an input's text (or its placeholder).
pub(super) struct Lines {
    lines: Vec<Line>,
    line_height: Pixels,
}

/// What [`Lines::shape`] shapes.
pub(super) struct Shaping<'a> {
    /// The text, or the placeholder.
    pub(super) text: SharedString,
    /// The style of every character.
    pub(super) run: TextRun,
    /// The IME composition to underline.
    pub(super) marked: Option<&'a Range<usize>>,
}

impl Lines {
    /// Shapes `shaping`'s text, a line per `\n`.
    pub(super) fn shape(shaping: &Shaping<'_>, window: &Window) -> Self {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let mut lines = Vec::new();
        let mut start = 0;
        for text in shaping.text.split('\n') {
            let range = start..start + text.len();
            let runs = runs(&shaping.run, &range, shaping.marked);
            let shaped = window.text_system().shape_line(
                SharedString::from(text.to_owned()),
                font_size,
                &runs,
                None,
            );
            lines.push(Line { start, shaped });
            start = range.end + 1;
        }
        Self { lines, line_height: window.line_height() }
    }

    /// The height of a line.
    pub(super) fn line_height(&self) -> Pixels {
        self.line_height
    }

    /// The widest line's width.
    pub(super) fn width(&self) -> Pixels {
        self.lines.iter().map(|line| line.shaped.width).fold(px(0.), Pixels::max)
    }

    /// The height of every line together.
    pub(super) fn height(&self) -> Pixels {
        self.line_height * self.lines.len() as f32
    }

    /// Which line holds `offset`.
    pub(super) fn line_of(&self, offset: usize) -> usize {
        self.lines.iter().rposition(|line| line.start <= offset).unwrap_or(0)
    }

    /// How many lines there are.
    pub(super) fn count(&self) -> usize {
        self.lines.len()
    }

    /// The top-left corner of the caret at `offset`.
    pub(super) fn position(&self, offset: usize) -> Point<Pixels> {
        let index = self.line_of(offset);
        let line = &self.lines[index];
        let x = line.shaped.x_for_index(offset.min(line.end()) - line.start);
        point(x, self.line_height * index as f32)
    }

    /// The offset closest to `x` on line `index` (clamped to the lines there are).
    pub(super) fn index_on_line(&self, index: usize, x: Pixels) -> usize {
        let line = &self.lines[index.min(self.lines.len() - 1)];
        line.start + line.shaped.closest_index_for_x(x)
    }

    /// The offset closest to `position`.
    pub(super) fn index_for(&self, position: Point<Pixels>) -> usize {
        let index = (position.y / self.line_height).floor().max(0.) as usize;
        self.index_on_line(index, position.x)
    }

    /// The rectangles that highlight `range`, a line each.
    pub(super) fn spans(&self, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut spans = Vec::new();
        for (index, line) in self.lines.iter().enumerate() {
            if range.end < line.start || range.start > line.end() {
                continue;
            }
            let from = range.start.max(line.start) - line.start;
            let to = range.end.min(line.end()) - line.start;
            let past_end = if range.end > line.end() { LINE_BREAK_WIDTH } else { px(0.) };
            let top = self.line_height * index as f32;
            spans.push(Bounds::from_corners(
                point(line.shaped.x_for_index(from), top),
                point(line.shaped.x_for_index(to) + past_end, top + self.line_height),
            ));
        }
        spans
    }

    /// Paints every line with its top-left corner at `origin`, a line height apart.
    pub(super) fn paint(&self, origin: Point<Pixels>, window: &mut Window, cx: &mut gpui::App) {
        for (index, line) in self.lines.iter().enumerate() {
            let top = origin + point(px(0.), self.line_height * index as f32);
            // A failed paint only loses this frame's text; the next frame tries again.
            line.shaped.paint(top, self.line_height, gpui::TextAlign::Left, None, window, cx).ok();
        }
    }
}

/// `run` cut to the line at `range` of the text, with the part of the `marked` composition on
/// it underlined.
fn runs(run: &TextRun, range: &Range<usize>, marked: Option<&Range<usize>>) -> Vec<TextRun> {
    let len = range.end - range.start;
    let marked = marked
        .map(|marked| marked.start.clamp(range.start, range.end)..marked.end.min(range.end))
        .filter(|marked| marked.start < marked.end)
        .map(|marked| marked.start - range.start..marked.end - range.start);
    let Some(marked) = marked else {
        return vec![TextRun { len, ..run.clone() }];
    };
    let underline =
        UnderlineStyle { color: Some(run.color), thickness: COMPOSITION_UNDERLINE, wavy: false };
    [
        TextRun { len: marked.start, ..run.clone() },
        TextRun { len: marked.end - marked.start, underline: Some(underline), ..run.clone() },
        TextRun { len: len - marked.end, ..run.clone() },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}
