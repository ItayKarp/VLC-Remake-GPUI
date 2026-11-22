"""A PNG of the finished pipeline: stages left to right, matrix cells stacked, steps in rows."""

from __future__ import annotations

import io
from dataclasses import dataclass

from PIL import Image, ImageDraw, ImageFont

from stages import Stage, format_duration

# Drawn at twice the size Discord shows it, so text stays sharp on high-density screens.
SCALE = 2
PADDING = 24 * SCALE
HEADER_HEIGHT = 56 * SCALE
TITLE_HEIGHT = 30 * SCALE
BOX_HEIGHT = 46 * SCALE
BOX_MIN_WIDTH = 92 * SCALE
BOX_TEXT_PADDING = 12 * SCALE
BOX_RADIUS = 8 * SCALE
BOX_GAP = 12 * SCALE
ROW_GAP = 10 * SCALE
COLUMN_GAP = 44 * SCALE
LINE_WIDTH = 2 * SCALE
NAME_SIZE = 15 * SCALE
DETAIL_SIZE = 12 * SCALE
HEADER_SIZE = 18 * SCALE

BACKGROUND = (43, 45, 49)
TEXT = (242, 243, 245)
MUTED = (181, 186, 193)
CONNECTOR = (78, 80, 88)
STATE_COLOUR = {
    "success": (35, 165, 90),
    "failure": (242, 63, 67),
    "unstable": (240, 178, 50),
    "aborted": (128, 132, 142),
    "running": (88, 101, 242),
}
IDLE_COLOUR = (78, 80, 88)
# How much of the state colour tints a box's fill; the rest is the background.
FILL_TINT = 0.22

FONT_PATHS = {
    False: ["/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", "/Library/Fonts/Arial Unicode.ttf"],
    True: ["/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf", "/Library/Fonts/Arial Unicode.ttf"],
}


@dataclass
class Box:
    """One drawn stage: its name, what it says under the name, and its width."""

    stage: Stage
    detail: str
    width: int


@dataclass
class Column:
    """A top-level stage: its title, and rows of boxes (one row per matrix cell)."""

    stage: Stage
    rows: list[list[Box]]
    width: int


def render(title: str, result: str, stages: list[Stage]) -> bytes:
    """The overview as PNG bytes: `title` and `result` across the top, then the stages."""
    fonts = _Fonts()
    columns = [_column(stage, fonts) for stage in stages]
    width = 2 * PADDING + sum(c.width for c in columns) + COLUMN_GAP * max(0, len(columns) - 1)
    width = max(width, 2 * PADDING + int(fonts.header.getlength(f"{title}   {result}")))
    tallest = max((len(c.rows) for c in columns), default=1)
    height = HEADER_HEIGHT + TITLE_HEIGHT + tallest * (BOX_HEIGHT + ROW_GAP) + PADDING
    image = Image.new("RGB", (width, height), BACKGROUND)
    draw = ImageDraw.Draw(image)
    _draw_header(draw, fonts, title, result, width)
    _draw_columns(draw, fonts, columns)
    output = io.BytesIO()
    image.save(output, format="PNG", optimize=True)
    return output.getvalue()


class _Fonts:
    def __init__(self) -> None:
        self.name = _font(NAME_SIZE, bold=True)
        self.detail = _font(DETAIL_SIZE, bold=False)
        self.header = _font(HEADER_SIZE, bold=True)


def _font(size: int, *, bold: bool) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for path in FONT_PATHS[bold]:
        try:
            return ImageFont.truetype(path, size)
        except OSError:
            continue
    return ImageFont.load_default(size)


def _column(stage: Stage, fonts: _Fonts) -> Column:
    cells = stage.children or [stage]
    rows = [[_box(cell, fonts), *(_box(step, fonts) for step in cell.children)] for cell in cells]
    widest_row = max(sum(box.width for box in row) + BOX_GAP * (len(row) - 1) for row in rows)
    title_width = int(fonts.name.getlength(stage.name))
    return Column(stage, rows, max(widest_row, title_width))


def _box(stage: Stage, fonts: _Fonts) -> Box:
    detail = format_duration(stage.duration_ms) if stage.has_run else stage.state.replace("_", " ")
    text_width = max(fonts.name.getlength(stage.name), fonts.detail.getlength(detail))
    return Box(stage, detail, max(BOX_MIN_WIDTH, int(text_width) + 2 * BOX_TEXT_PADDING))


def _draw_header(draw: ImageDraw.ImageDraw, fonts: _Fonts, title: str, result: str, width: int) -> None:
    middle = HEADER_HEIGHT // 2
    draw.text((PADDING, middle), title, font=fonts.header, fill=TEXT, anchor="lm")
    draw.text((width - PADDING, middle), result, font=fonts.header, fill=MUTED, anchor="rm")


def _draw_columns(draw: ImageDraw.ImageDraw, fonts: _Fonts, columns: list[Column]) -> None:
    left = PADDING
    first_row_middle = HEADER_HEIGHT + TITLE_HEIGHT + BOX_HEIGHT // 2
    for index, column in enumerate(columns):
        if index:
            start = (left - COLUMN_GAP, first_row_middle)
            draw.line([start, (left, first_row_middle)], fill=CONNECTOR, width=LINE_WIDTH)
        colour = _colour(column.stage.state)
        draw.text((left, HEADER_HEIGHT), column.stage.name, font=fonts.name, fill=colour, anchor="lt")
        top = HEADER_HEIGHT + TITLE_HEIGHT
        for row in column.rows:
            _draw_row(draw, fonts, row, left, top)
            top += BOX_HEIGHT + ROW_GAP
        left += column.width + COLUMN_GAP


def _draw_row(draw: ImageDraw.ImageDraw, fonts: _Fonts, row: list[Box], left: int, top: int) -> None:
    middle = top + BOX_HEIGHT // 2
    for index, box in enumerate(row):
        if index:
            draw.line([(left - BOX_GAP, middle), (left, middle)], fill=CONNECTOR, width=LINE_WIDTH)
        _draw_box(draw, fonts, box, left, top)
        left += box.width + BOX_GAP


def _draw_box(draw: ImageDraw.ImageDraw, fonts: _Fonts, box: Box, left: int, top: int) -> None:
    colour = _colour(box.stage.state)
    fill = tuple(round(c * FILL_TINT + b * (1 - FILL_TINT)) for c, b in zip(colour, BACKGROUND))
    shape = [left, top, left + box.width, top + BOX_HEIGHT]
    draw.rounded_rectangle(shape, radius=BOX_RADIUS, fill=fill, outline=colour, width=LINE_WIDTH)
    centre = left + box.width // 2
    draw.text((centre, top + BOX_HEIGHT * 0.36), box.stage.name, font=fonts.name, fill=TEXT, anchor="mm")
    draw.text((centre, top + BOX_HEIGHT * 0.72), box.detail, font=fonts.detail, fill=MUTED, anchor="mm")


def _colour(state: str) -> tuple[int, int, int]:
    return STATE_COLOUR.get(state, IDLE_COLOUR)
