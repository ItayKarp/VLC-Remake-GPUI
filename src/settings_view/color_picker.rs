//! A color field's square and the palette it opens (S5.8.3; the picker isn't designed). The
//! square shows the saved color; clicking it, or Enter or Space on it, opens sixteen colors that
//! read well as subtitles under it, the saved one ringed. Picking one saves it and closes the
//! palette, as does Escape or a click outside. While it's open the arrow keys, Home and End move
//! a focus ring over the colors and Enter or Space picks the ringed one (S9.1.1). Any other color can still be typed in the field.

use gpui::{
    Context, FocusHandle, Hsla, IntoElement, KeyDownEvent, MouseDownEvent, Render, SharedString,
    Window, div, prelude::*, px, rgb,
};

use crate::i18n::DirectionalStyle;
use crate::settings::{
    self,
    registry::{self, SettingDef, Value},
};
use crate::theme::{self, Palette};
use crate::ui::focus::{self, Axis, Layout};
use crate::ui::metrics;
use crate::ui::popover::{self, BoundsTracker, Side};

/// The palette, white to black, then the colors.
const COLORS: [u32; 16] = [
    0xFF_FFFF, 0xC0_C0C0, 0x80_8080, 0x00_0000, 0xFF_FF00, 0xFF_D700, 0xFF_A500, 0xFF_0000,
    0xFF_69B4, 0xFF_00FF, 0x80_0080, 0x00_00FF, 0x00_BFFF, 0x00_FFFF, 0x00_FF00, 0x00_8000,
];
/// Colors in a palette row.
const PER_ROW: usize = 8;
/// The square beside the field, and a palette swatch.
const SQUARE: f32 = 24.;
const SWATCH: f32 = 28.;
const GAP: f32 = 8.;

/// A color row's square and palette.
pub(super) struct ColorPicker {
    def: &'static SettingDef,
    open: bool,
    /// The palette color the arrow keys are on, while it's open.
    highlighted: Option<usize>,
    trigger: BoundsTracker,
    focus_handle: FocusHandle,
}

impl ColorPicker {
    /// The square of the color row `def`.
    pub(super) fn new(def: &'static SettingDef, cx: &mut Context<Self>) -> Self {
        Self {
            def,
            open: false,
            highlighted: None,
            trigger: BoundsTracker::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// The saved color, if it is one.
    fn saved(&self, cx: &Context<Self>) -> Option<u32> {
        match self.def.value(settings::get(cx)) {
            Value::Text(text) => registry::hex_color(&text),
            _ => None,
        }
    }

    fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        let enabled = self.def.is_enabled(settings::get(cx));
        self.open = open && enabled;
        let saved = self.saved(cx);
        self.highlighted = COLORS.iter().position(|&color| Some(color) == saved);
        cx.notify();
    }

    fn pick(&mut self, color: u32, cx: &mut Context<Self>) {
        let def = self.def;
        if let Ok(value) = def.accept(Value::Text(format!("#{color:06X}"))) {
            settings::update(cx, |settings| (def.binding.set)(settings, value));
        }
        self.set_open(false, cx);
    }

    fn on_key(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.open
            && let Some(movement) = focus::group_key(event, Axis::Grid)
        {
            let layout = Layout::grid(PER_ROW, COLORS.len() / PER_ROW);
            self.highlighted =
                focus::step(self.highlighted, COLORS.len(), movement, layout, |_| true);
            cx.stop_propagation();
            cx.notify();
            return;
        }
        match event.keystroke.key.as_str() {
            "enter" | "space" => match self.highlighted.filter(|_| self.open) {
                Some(index) => self.pick(COLORS[index], cx),
                None => self.set_open(!self.open, cx),
            },
            "escape" if self.open => {
                self.set_open(false, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn close_outside(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !self.trigger.contains(event.position) {
            self.set_open(false, cx);
        }
    }
}

impl Render for ColorPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let saved = self.saved(cx);
        let label = SharedString::from(choose_label(self.def.label));
        let square = div()
            .id("color-square")
            .relative()
            .flex_none()
            .size(px(SQUARE + 4.))
            .p(px(2.))
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .cursor_pointer()
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .aria_label(label)
            .aria_expanded(self.open)
            .on_key_down(cx.listener(Self::on_key))
            .on_click(cx.listener(|this, _, _, cx| this.set_open(!this.open, cx)))
            .child(self.trigger.element())
            .child(color_box(saved, SQUARE, p.divider));
        let opened = self.open.then(|| self.trigger.bounds()).flatten().map(|bounds| {
            let marks = Marks { saved, highlighted: self.highlighted.map(|index| COLORS[index]) };
            popover::attached(bounds, Side::Below, 4., 1, palette(marks, &p, window, cx))
        });
        div().child(focus::ring(square, &p)).children(opened)
    }
}

/// Which palette colors are marked: the saved one, and the one the arrow keys are on.
#[derive(Clone, Copy)]
struct Marks {
    saved: Option<u32>,
    highlighted: Option<u32>,
}

/// The sixteen colors under the square.
fn palette(
    marks: Marks,
    p: &Palette,
    window: &Window,
    cx: &Context<ColorPicker>,
) -> impl IntoElement {
    let rows = COLORS.chunks(PER_ROW).map(|row| {
        div()
            .flex_row_start()
            .gap(px(GAP))
            .children(row.iter().map(|&color| swatch(color, marks, p, cx)))
    });
    popover::surface(p, window)
        .id("color-palette")
        .p(px(12.))
        .flex_row_start()
        .flex_col()
        .gap(px(GAP))
        .on_mouse_down_out(cx.listener(ColorPicker::close_outside))
        .children(rows)
}

/// One palette color; the arrow keys' one is ringed in the focus color, else the saved one in
/// the accent.
fn swatch(color: u32, marks: Marks, p: &Palette, cx: &Context<ColorPicker>) -> impl IntoElement {
    let ring = if marks.highlighted == Some(color) {
        p.focus_ring
    } else if marks.saved == Some(color) {
        p.accent
    } else {
        gpui::transparent_black()
    };
    div()
        .id(SharedString::from(format!("swatch-{color:06X}")))
        .p(px(2.))
        .rounded(px(metrics::RADIUS))
        .border_2()
        .border_color(ring)
        .cursor_pointer()
        .aria_label(SharedString::from(format!("#{color:06X}")))
        .on_click(cx.listener(move |this, _, _, cx| this.pick(color, cx)))
        .child(color_box(Some(color), SWATCH, p.divider))
}

/// A filled square of `color`, outlined so white and black show on any background.
fn color_box(color: Option<u32>, size: f32, outline: Hsla) -> impl IntoElement {
    div()
        .size(px(size))
        .rounded(px(2.))
        .border_1()
        .border_color(outline)
        .when_some(color, |square, color| square.bg(rgb(color)))
}

/// The square's accessible name, "Choose text color". English lowercases the label to fit
/// it mid-sentence; a translation keeps the label as its translator wrote it.
fn choose_label(label: &'static str) -> String {
    let shown = crate::i18n::text(label);
    let setting = if shown == label { shown.to_lowercase() } else { shown.to_owned() };
    tr!("Choose {setting}", setting)
}
