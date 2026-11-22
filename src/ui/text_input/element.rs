//! The element that paints the text, selection, caret and IME underline, and registers the
//! platform input handler.

use gpui::{
    App, Bounds, ContentMask, ElementId, ElementInputHandler, Entity, GlobalElementId,
    InspectorElementId, LayoutId, PaintQuad, Pixels, Point, SharedString, Style, TextRun, Window,
    fill, point, prelude::*, px, relative, size,
};

use super::layout::{Lines, Shaping};
use super::{Scrolling, TextInput};
use crate::theme::{self, Palette};

/// Room kept between the caret and the right edge, so the caret isn't clipped.
const CARET_MARGIN: Pixels = px(2.);

/// How wide the caret is drawn.
const CARET_WIDTH: Pixels = px(1.5);

/// The placeholder is drawn fainter than muted text, so it never reads as a value.
const PLACEHOLDER_OPACITY: f32 = 0.7;

/// A selection in an unfocused input stays visible, but only just.
const UNFOCUSED_SELECTION_OPACITY: f32 = 0.15;

/// Paints a [`TextInput`]'s text.
pub(super) struct TextElement {
    /// The input whose state is painted.
    pub(super) input: Entity<TextInput>,
}

/// What [`TextElement::prepaint`] works out for [`TextElement::paint`].
pub(super) struct PrepaintState {
    lines: Option<Lines>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    scroll: Point<Pixels>,
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
        style.size.height = if self.input.read(cx).state.is_multi_line() {
            relative(1.).into()
        } else {
            window.line_height().into()
        };
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
        let lines = shape_text(input, &palette, window);
        let caret =
            if state.text().is_empty() { Point::default() } else { lines.position(state.cursor()) };
        let scroll = scroll_into_view(input, &lines, caret, bounds);
        let origin = bounds.origin - scroll;

        let selection = state.selection();
        let (selection, cursor) = if selection.is_empty() || state.text().is_empty() {
            let caret = Bounds::new(origin + caret, size(CARET_WIDTH, lines.line_height()));
            (Vec::new(), focused.then(|| fill(caret, palette.accent_fg)))
        } else {
            let color = if focused {
                palette.selection
            } else {
                palette.selection.opacity(UNFOCUSED_SELECTION_OPACITY)
            };
            let spans = lines.spans(&selection).into_iter();
            (
                spans
                    .map(|span| fill(Bounds::new(span.origin + origin, span.size), color))
                    .collect(),
                None,
            )
        };
        PrepaintState { lines: Some(lines), cursor, selection, scroll }
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
        let lines = prepaint.lines.take();
        let scroll = prepaint.scroll;
        self.input.update(cx, |input, _| {
            input.last_layout = lines;
            input.last_bounds = Some(bounds);
            input.scroll = scroll;
            input.scrolling = Scrolling::Free;
        });
    }
}

/// Paints the selection under the text and the caret over it.
fn paint_contents(
    prepaint: &mut PrepaintState,
    bounds: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    for span in prepaint.selection.drain(..) {
        window.paint_quad(span);
    }
    if let Some(lines) = prepaint.lines.as_ref() {
        lines.paint(bounds.origin - prepaint.scroll, window, cx);
    }
    if let Some(cursor) = prepaint.cursor.take() {
        window.paint_quad(cursor);
    }
}

/// Shapes the input's text, or its placeholder when empty, with any IME composition underlined.
fn shape_text(input: &TextInput, palette: &Palette, window: &Window) -> Lines {
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
    Lines::shape(&Shaping { text, run, marked }, window)
}

/// The scroll that keeps the caret at `caret` in view after it moved, or else the last scroll,
/// kept within the text.
fn scroll_into_view(
    input: &TextInput,
    lines: &Lines,
    caret: Point<Pixels>,
    bounds: Bounds<Pixels>,
) -> Point<Pixels> {
    let follow = input.scrolling == Scrolling::ToCaret;
    let x = Axis { scroll: input.scroll.x, view: bounds.size.width, content: lines.width() };
    let y = Axis { scroll: input.scroll.y, view: bounds.size.height, content: lines.height() };
    point(
        x.scroll_to(follow.then_some((caret.x, CARET_MARGIN)), CARET_MARGIN),
        y.scroll_to(follow.then_some((caret.y, lines.line_height())), px(0.)),
    )
}

/// One direction's scrolling: where it is, how much shows, and how much there is.
struct Axis {
    scroll: Pixels,
    view: Pixels,
    content: Pixels,
}

impl Axis {
    /// The scroll that shows `caret` (its start and its size) when given, else the current one,
    /// within the content plus `margin`.
    fn scroll_to(&self, caret: Option<(Pixels, Pixels)>, margin: Pixels) -> Pixels {
        let scroll = match caret {
            Some((at, extent)) if at + extent - self.scroll > self.view => at + extent - self.view,
            Some((at, _)) if at < self.scroll => at,
            _ => self.scroll,
        };
        scroll.max(px(0.)).min((self.content - self.view + margin).max(px(0.)))
    }
}
