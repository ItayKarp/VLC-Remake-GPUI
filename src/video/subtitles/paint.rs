//! Shaping and painting an [`Overlay`]'s cues with GPUI's text system, which shapes right to
//! left and mixed scripts, CJK and emoji with the OS's own text engine. Each cue is shaped once
//! per color: the outline is its text painted in the outline color around each letter, the
//! shadow its text offset below and right, then the text itself on top.

use std::f32::consts::TAU;
use std::sync::OnceLock;

use gpui::{
    App, Bounds, Font, FontStyle, FontWeight, Hsla, Pixels, Point, SharedString, TextAlign,
    TextRun, UnderlineStyle, Window, WrappedLine, black, font, point, px, rgb, size,
};

use super::{Band, Overlay, Placing, font_size, stack};
use crate::engine::cues::{Cue, Emphasis, Placement, Span};
use crate::engine::subtitle_style::OPAQUE;

/// The family GPUI resolves to the system's interface font.
const SYSTEM_FONT: &str = ".SystemUIFont";
/// The height of a line of text, per font size.
const LINE_HEIGHT: f32 = 1.2;
/// The outline's width per font size, for each step of libVLC's outline thickness (2, 4, 6).
const OUTLINE_PER_STEP: f32 = 1. / 80.;
/// How many copies around each letter make the outline.
const OUTLINE_COPIES: u16 = 12;
/// How far the shadow falls, per font size (libVLC's `freetype-shadow-distance`).
const SHADOW_DISTANCE: f32 = 0.06;
/// The shadow's opacity, as libVLC's default shadow (128 of 255).
const SHADOW_OPACITY: f32 = 0.5;
/// An underline's thickness, per font size.
const UNDERLINE_PER_SIZE: f32 = 1. / 16.;

/// Paints `overlay` at `placing`.
pub fn paint(overlay: &Overlay, placing: &Placing, window: &mut Window, cx: &mut App) {
    let band = Band::new(overlay.area, placing);
    let font_size =
        font_size(overlay.style.size, placing.picture.size.height, placing.video_height);
    let painter = Painter {
        overlay,
        family: family(&overlay.style.font, cx),
        font_size,
        line_height: font_size * LINE_HEIGHT,
        width: band.width,
    };
    let left = band.center - band.width / 2.;
    for placement in [Placement::Bottom, Placement::Middle, Placement::Top] {
        let cues = overlay.cues.iter().filter(|cue| cue.placement == placement);
        let shaped: Vec<Shaped> = cues.filter_map(|cue| painter.shape(cue, window)).collect();
        let heights: Vec<Pixels> = shaped.iter().map(|shaped| shaped.height).collect();
        for (shaped, top) in shaped.iter().zip(stack(placement, &heights, &band)) {
            painter.paint(shaped, point(left, top), window, cx);
        }
    }
}

/// One cue shaped in its three colors.
struct Shaped {
    text: Vec<WrappedLine>,
    outline: Option<Vec<WrappedLine>>,
    shadow: Option<Vec<WrappedLine>>,
    height: Pixels,
}

/// What every cue of one overlay is shaped and painted with.
struct Painter<'a> {
    overlay: &'a Overlay,
    family: SharedString,
    font_size: Pixels,
    line_height: Pixels,
    /// The band's width, which lines wrap at and center in.
    width: Pixels,
}

impl Painter<'_> {
    /// Shapes `cue`; `None` when the text system can't.
    fn shape(&self, cue: &Cue, window: &Window) -> Option<Shaped> {
        let style = &self.overlay.style;
        let outline = (self.outline_width() > px(0.)).then(|| self.color(style.outline_color));
        let shadow = style.shadow.then(|| black().opacity(SHADOW_OPACITY * self.opacity()));
        let text = self.lines(cue, None, window)?;
        let height = text.iter().map(|line| line.size(self.line_height).height).sum();
        Some(Shaped {
            outline: outline.and_then(|color| self.lines(cue, Some(color), window)),
            shadow: shadow.and_then(|color| self.lines(cue, Some(color), window)),
            text,
            height,
        })
    }

    /// `cue` shaped and wrapped, in its own colors or all in `color`.
    fn lines(&self, cue: &Cue, color: Option<Hsla>, window: &Window) -> Option<Vec<WrappedLine>> {
        let runs: Vec<TextRun> = cue.spans.iter().map(|span| self.run(span, color)).collect();
        let text = SharedString::from(cue.text());
        let shaped =
            window.text_system().shape_text(text, self.font_size, &runs, Some(self.width), None);
        match shaped {
            Ok(lines) => Some(lines.into_iter().collect()),
            Err(error) => {
                tracing::debug!("subtitles: couldn't shape a cue: {error}");
                None
            }
        }
    }

    fn run(&self, span: &Span, color: Option<Hsla>) -> TextRun {
        let emphasis = span.emphasis;
        let color = color.unwrap_or_else(|| self.text_color(span));
        let underline = emphasis.contains(Emphasis::UNDERLINE).then_some(UnderlineStyle {
            thickness: self.font_size * UNDERLINE_PER_SIZE,
            color: None,
            wavy: false,
        });
        TextRun {
            len: span.text.len(),
            font: self.font(emphasis),
            color,
            background_color: None,
            underline,
            strikethrough: None,
        }
    }

    fn font(&self, emphasis: Emphasis) -> Font {
        let bold = self.overlay.style.bold || emphasis.contains(Emphasis::BOLD);
        let italic = emphasis.contains(Emphasis::ITALIC);
        Font {
            weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL },
            style: if italic { FontStyle::Italic } else { FontStyle::Normal },
            ..font(self.family.clone())
        }
    }

    /// A span's color: the file's while Embedded Sub Format is on, else the user's.
    fn text_color(&self, span: &Span) -> Hsla {
        let file = span.color.filter(|_| self.overlay.file_colors);
        self.color(file.unwrap_or(self.overlay.style.color))
    }

    /// `0xRRGGBB` at the text's opacity.
    fn color(&self, color: u32) -> Hsla {
        Hsla::from(rgb(color)).opacity(self.opacity())
    }

    fn opacity(&self) -> f32 {
        f32::from(self.overlay.style.opacity) / f32::from(OPAQUE)
    }

    fn outline_width(&self) -> Pixels {
        self.font_size * (self.overlay.style.outline.libvlc_value() as f32 * OUTLINE_PER_STEP)
    }

    /// Paints `shaped` with its top left at `origin`: shadow, outline, then text.
    fn paint(&self, shaped: &Shaped, origin: Point<Pixels>, window: &mut Window, cx: &mut App) {
        if let Some(shadow) = &shaped.shadow {
            let distance = self.font_size * SHADOW_DISTANCE + self.outline_width();
            self.paint_lines(shadow, origin + point(distance, distance), window, cx);
        }
        if let Some(outline) = &shaped.outline {
            let width = f32::from(self.outline_width());
            for copy in 0..OUTLINE_COPIES {
                let angle = TAU * f32::from(copy) / f32::from(OUTLINE_COPIES);
                let offset = point(px(width * angle.cos()), px(width * angle.sin()));
                self.paint_lines(outline, origin + offset, window, cx);
            }
        }
        self.paint_lines(&shaped.text, origin, window, cx);
    }

    /// Paints one shaped cue's lines, each centered in the band, from `origin` down.
    fn paint_lines(
        &self,
        lines: &[WrappedLine],
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut top = origin.y;
        for line in lines {
            let height = line.size(self.line_height).height;
            let at = point(origin.x, top);
            let bounds = Bounds::new(at, size(self.width, height));
            let painted =
                line.paint(at, self.line_height, TextAlign::Center, Some(bounds), window, cx);
            if let Err(error) = painted {
                tracing::debug!("subtitles: couldn't paint a line: {error}");
            }
            top += height;
        }
    }
}

/// The family to draw `font` in: the installed family it names (in any case), or the system's
/// when it names none or one this machine lacks (Settings says so in the Font row).
fn family(font: &str, cx: &App) -> SharedString {
    static INSTALLED: OnceLock<Vec<String>> = OnceLock::new();
    let font = font.trim();
    if font.is_empty() {
        return SYSTEM_FONT.into();
    }
    let installed = INSTALLED.get_or_init(|| cx.text_system().all_font_names());
    installed
        .iter()
        .find(|name| name.eq_ignore_ascii_case(font))
        .map_or_else(|| SYSTEM_FONT.into(), |name| SharedString::from(name.clone()))
}
