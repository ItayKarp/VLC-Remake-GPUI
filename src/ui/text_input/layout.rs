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

/// What a masked input draws for each character.
const BULLET: char = '•';

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
    shaped: Vec<Line>,
    line_height: Pixels,
    /// For a masked input, how its text's offsets map to the bullets drawn.
    mask: Option<Mask>,
}

/// What [`Lines::shape`] shapes.
pub(super) struct Shaping<'a> {
    /// The text, or the placeholder.
    pub(super) text: SharedString,
    /// The style of every character.
    pub(super) run: TextRun,
    /// The IME composition to underline.
    pub(super) marked: Option<&'a Range<usize>>,
    /// Whether to draw a bullet per character instead (a single-line password).
    pub(super) masked: bool,
}

impl Lines {
    /// Shapes `shaping`'s text, a line per `\n`.
    pub(super) fn shape(shaping: &Shaping<'_>, window: &Window) -> Self {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let mask = shaping.masked.then(|| Mask::new(&shaping.text));
        let text = match &mask {
            Some(mask) => SharedString::from(mask.bullets()),
            None => shaping.text.clone(),
        };
        let mut lines = Vec::new();
        let mut start = 0;
        for text in text.split('\n') {
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
        Self { shaped: lines, line_height: window.line_height(), mask }
    }

    /// Where the text's `offset` is in what was shaped.
    fn shown(&self, offset: usize) -> usize {
        self.mask.as_ref().map_or(offset, |mask| mask.shown(offset))
    }

    /// The height of a line.
    pub(super) fn line_height(&self) -> Pixels {
        self.line_height
    }

    /// The widest line's width.
    pub(super) fn width(&self) -> Pixels {
        self.shaped.iter().map(|line| line.shaped.width).fold(px(0.), Pixels::max)
    }

    /// The height of every line together.
    pub(super) fn height(&self) -> Pixels {
        self.line_height * self.shaped.len() as f32
    }

    /// Which line holds `offset`.
    pub(super) fn line_of(&self, offset: usize) -> usize {
        let offset = self.shown(offset);
        self.shaped.iter().rposition(|line| line.start <= offset).unwrap_or(0)
    }

    /// How many lines there are.
    pub(super) fn count(&self) -> usize {
        self.shaped.len()
    }

    /// The top-left corner of the caret at `offset`.
    pub(super) fn position(&self, offset: usize) -> Point<Pixels> {
        let index = self.line_of(offset);
        let offset = self.shown(offset);
        let line = &self.shaped[index];
        let x = line.shaped.x_for_index(offset.min(line.end()) - line.start);
        point(x, self.line_height * index as f32)
    }

    /// The offset closest to `x` on line `index` (clamped to the lines there are).
    pub(super) fn index_on_line(&self, index: usize, x: Pixels) -> usize {
        let line = &self.shaped[index.min(self.shaped.len() - 1)];
        let shown = line.start + line.shaped.closest_index_for_x(x);
        self.mask.as_ref().map_or(shown, |mask| mask.text(shown))
    }

    /// The offset closest to `position`.
    pub(super) fn index_for(&self, position: Point<Pixels>) -> usize {
        let index = (position.y / self.line_height).floor().max(0.) as usize;
        self.index_on_line(index, position.x)
    }

    /// The rectangles that highlight `range`, a line each.
    pub(super) fn spans(&self, range: &Range<usize>) -> Vec<Bounds<Pixels>> {
        let range = &(self.shown(range.start)..self.shown(range.end));
        let mut spans = Vec::new();
        for (index, line) in self.shaped.iter().enumerate() {
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
        for (index, line) in self.shaped.iter().enumerate() {
            let top = origin + point(px(0.), self.line_height * index as f32);
            // A failed paint only loses this frame's text; the next frame tries again.
            line.shaped.paint(top, self.line_height, gpui::TextAlign::Left, None, window, cx).ok();
        }
    }
}

/// A masked text's character offsets, to map them to its bullets and back.
struct Mask {
    /// Where each character starts, then the text's length.
    starts: Vec<usize>,
}

impl Mask {
    fn new(text: &str) -> Self {
        let starts = text.char_indices().map(|(start, _)| start).chain([text.len()]);
        Self { starts: starts.collect() }
    }

    /// A bullet per character.
    fn bullets(&self) -> String {
        BULLET.to_string().repeat(self.starts.len() - 1)
    }

    /// Where the text's `offset` is among the bullets.
    fn shown(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start < offset) * BULLET.len_utf8()
    }

    /// The text's offset at the bullets' `shown`.
    fn text(&self, shown: usize) -> usize {
        let index = (shown / BULLET.len_utf8()).min(self.starts.len() - 1);
        self.starts[index]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mask_maps_characters_to_bullets_and_back() {
        let mask = Mask::new("aé€");
        assert_eq!(mask.bullets(), "•••");
        let bullet = BULLET.len_utf8();
        // "a" is one byte, "é" two and "€" three: each is one bullet.
        assert_eq!([0, 1, 3, 6].map(|offset| mask.shown(offset)), [0, 1, 2, 3].map(|n| n * bullet));
        assert_eq!([0, 1, 2, 3].map(|n| mask.text(n * bullet)), [0, 1, 3, 6]);
        assert_eq!(mask.text(100), 6);
    }
}
