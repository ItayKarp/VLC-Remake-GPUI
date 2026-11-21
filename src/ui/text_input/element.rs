//! The element that paints the text, selection, caret and IME underline, and registers the
//! platform input handler.

use std::ops::Range;

use gpui::{
    App, Bounds, ContentMask, ElementId, ElementInputHandler, Entity, GlobalElementId,
    InspectorElementId, LayoutId, PaintQuad, Pixels, ShapedLine, SharedString, Style, TextAlign,
    TextRun, UnderlineStyle, Window, fill, point, prelude::*, px, relative, size,
};

use super::TextInput;
use crate::theme::{self, Palette};

/// Room kept between the caret and the right edge, so the caret isn't clipped.
const CARET_MARGIN: Pixels = px(2.);

/// How wide the caret is drawn.
const CARET_WIDTH: Pixels = px(1.5);

/// Thickness of the underline under an IME composition.
const COMPOSITION_UNDERLINE: Pixels = px(1.);

/// The placeholder is drawn fainter than muted text, so it never reads as a value.
const PLACEHOLDER_OPACITY: f32 = 0.7;

/// A selection in an unfocused input stays visible, but only just.
const UNFOCUSED_SELECTION_OPACITY: f32 = 0.15;

/// Paints a [`TextInput`]'s line of text.
pub(super) struct TextElement {
    /// The input whose state is painted.
    pub(super) input: Entity<TextInput>,
}

/// What [`TextElement::prepaint`] works out for [`TextElement::paint`].
pub(super) struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> PrepaintState {
        let palette = theme::palette(window, cx);
        let input = self.input.read(cx);
        let state = &input.state;
        let focused = input.focus_handle.is_focused(window);
        let line = shape_text(input, &palette, window);
        let caret_x =
            if state.text().is_empty() { px(0.) } else { line.x_for_index(state.cursor()) };
        let scroll_x = scroll_to_caret(input.scroll_x, caret_x, bounds.size.width, line.width);

        let selection = state.selection();
        let (selection, cursor) = if selection.is_empty() || state.text().is_empty() {
            let caret = Bounds::new(
                point(bounds.left() + caret_x - scroll_x, bounds.top()),
                size(CARET_WIDTH, bounds.size.height),
            );
            (None, focused.then(|| fill(caret, palette.accent_fg)))
        } else {
            let span = Span {
                left: line.x_for_index(selection.start) - scroll_x,
                right: line.x_for_index(selection.end) - scroll_x,
            };
            let color = if focused {
                palette.selection
            } else {
                palette.selection.opacity(UNFOCUSED_SELECTION_OPACITY)
            };
            (Some(fill(span.bounds(bounds), color)), None)
        };
        PrepaintState { line: Some(line), cursor, selection, scroll_x }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut (),
        prepaint: &mut PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            paint_contents(prepaint, bounds, window, cx);
        });
        let line = prepaint.line.take();
        let scroll_x = prepaint.scroll_x;
        self.input.update(cx, |input, _| {
            input.last_layout = line;
            input.last_bounds = Some(bounds);
            input.scroll_x = scroll_x;
        });
    }
}

/// A horizontal stretch of the line, relative to the element's left edge.
struct Span {
    left: Pixels,
    right: Pixels,
}

impl Span {
    /// The span's rectangle, the full height of `bounds`.
    fn bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        Bounds::from_corners(
            point(bounds.left() + self.left, bounds.top()),
            point(bounds.left() + self.right, bounds.bottom()),
        )
    }
}

/// Paints the selection under the text and the caret over it.
fn paint_contents(
    prepaint: &mut PrepaintState,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(selection) = prepaint.selection.take() {
        window.paint_quad(selection);
    }
    if let Some(line) = prepaint.line.as_ref() {
        let origin = point(bounds.left() - prepaint.scroll_x, bounds.top());
        // A failed paint only loses this frame's text; the next frame tries again.
        line.paint(origin, window.line_height(), TextAlign::Left, None, window, cx).ok();
    }
    if let Some(cursor) = prepaint.cursor.take() {
        window.paint_quad(cursor);
    }
}

/// Shapes the input's text, or its placeholder when empty, with any IME composition underlined.
fn shape_text(input: &TextInput, palette: &Palette, window: &Window) -> ShapedLine {
    let style = window.text_style();
    let value = input.state.text();
    let (text, color) = if value.is_empty() {
        (input.placeholder.clone(), palette.text_muted.opacity(PLACEHOLDER_OPACITY))
    } else {
        (SharedString::from(value.to_owned()), style.color)
    };
    let run = TextRun {
        len: text.len(),
        font: style.font(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let marked = input.state.marked().filter(|_| !value.is_empty());
    let runs = match marked {
        Some(marked) => composition_runs(&run, marked),
        None => vec![run],
    };
    let font_size = style.font_size.to_pixels(window.rem_size());
    window.text_system().shape_line(text, font_size, &runs, None)
}

/// Splits `run` in three so the `marked` composition in the middle is underlined.
fn composition_runs(run: &TextRun, marked: &Range<usize>) -> Vec<TextRun> {
    let underline =
        UnderlineStyle { color: Some(run.color), thickness: COMPOSITION_UNDERLINE, wavy: false };
    [
        TextRun { len: marked.start, ..run.clone() },
        TextRun { len: marked.end - marked.start, underline: Some(underline), ..run.clone() },
        TextRun { len: run.len - marked.end, ..run.clone() },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

/// The horizontal scroll that keeps the caret at `caret_x` in view, starting from `scroll_x`.
fn scroll_to_caret(scroll_x: Pixels, caret_x: Pixels, width: Pixels, line_width: Pixels) -> Pixels {
    let scroll_x = if caret_x - scroll_x > width - CARET_MARGIN {
        caret_x - width + CARET_MARGIN
    } else if caret_x < scroll_x {
        caret_x
    } else {
        scroll_x
    };
    scroll_x.max(px(0.)).min((line_width - width + CARET_MARGIN).max(px(0.)))
}
