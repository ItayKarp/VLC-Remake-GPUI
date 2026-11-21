//! The gallery's stateful components (entities), built once when the window opens.

use gpui::{AnyElement, App, Context, Entity, Hsla, SharedString, Window, div, prelude::*, px};

use super::Gallery;
use crate::theme::{self, Palette};
use crate::ui::dropdown::Dropdown;
use crate::ui::list::{self, Columns, Shape, VirtualList};
use crate::ui::modal::ModalLayer;
use crate::ui::slider::{Slider, SliderModel};
use crate::ui::text_input::TextInput;
use crate::ui::toast::Toasts;

const STEREO_MODES: [&str; 6] =
    ["Mono", "Stereo", "Reverse stereo", "Left", "Right", "Dolby Surround"];

/// Enough presets that the dropdown has to scroll.
const EQ_PRESETS: [&str; 18] = [
    "Default",
    "Classical",
    "Club",
    "Dance",
    "Full Bass",
    "Full Bass and Treble",
    "Full Treble",
    "Headphones",
    "Large Hall",
    "Live",
    "Party",
    "Pop",
    "Reggae",
    "Rock",
    "Ska",
    "Soft",
    "Soft Rock",
    "Techno",
];

/// The starting gain of each equalizer band, in dB, so the bands show a curve.
const EQ_GAINS: [f64; 10] = [3., 5., 2., 0., -2., -3., 0., 2., 4., 6.];

/// Enough items to show that only the visible ones are rendered.
const MEDIA_ITEMS: usize = 10_000;

/// The gallery's component entities.
pub(super) struct Widgets {
    pub(super) dropdown: Entity<Dropdown>,
    pub(super) long_dropdown: Entity<Dropdown>,
    pub(super) sliders: Sliders,
    pub(super) name: Entity<TextInput>,
    pub(super) volume_field: Entity<TextInput>,
    pub(super) grid: Entity<VirtualList>,
    pub(super) modals: Entity<ModalLayer>,
    pub(super) toasts: Entity<Toasts>,
}

/// The gallery's sliders, one per slider variant.
pub(super) struct Sliders {
    pub(super) saturation: Entity<Slider>,
    pub(super) panel_saturation: Entity<Slider>,
    pub(super) speed: Entity<Slider>,
    pub(super) sepia: Entity<Slider>,
    pub(super) delay: Entity<Slider>,
    pub(super) eq: Vec<Entity<Slider>>,
}

impl Widgets {
    /// Builds every component entity the gallery shows.
    pub(super) fn new(window: &mut Window, cx: &mut Context<Gallery>) -> Self {
        Self {
            dropdown: cx
                .new(|cx| Dropdown::new("Stereo mode", shared(&STEREO_MODES), 0, window, cx)),
            long_dropdown: cx
                .new(|cx| Dropdown::new("Equalizer preset", shared(&EQ_PRESETS), 0, window, cx)),
            sliders: Sliders::new(window, cx),
            name: cx.new(|cx| {
                TextInput::new(window, cx)
                    .with_placeholder("Playlist name")
                    .with_label("Playlist name")
            }),
            volume_field: cx.new(|cx| {
                TextInput::new(window, cx).with_text("200%").with_label("Starting volume")
            }),
            grid: cx.new(|cx| media_grid(window, cx)),
            modals: cx.new(|_| ModalLayer::new()),
            toasts: cx.new(|_| Toasts::new()),
        }
    }
}

impl Sliders {
    fn new(window: &mut Window, cx: &mut Context<Gallery>) -> Self {
        Self {
            saturation: cx.new(|cx| saturation(window, cx)),
            panel_saturation: cx.new(|cx| saturation(window, cx)),
            speed: cx.new(|cx| {
                Slider::new("Playback speed", SliderModel::new(0.25, 4., 0.05, 1.), window, cx)
                    .accent()
                    .format(|v| format!("{v:.2}×"))
            }),
            sepia: cx.new(|cx| {
                Slider::new("Sepia intensity", SliderModel::new(0., 255., 1., 120.), window, cx)
                    .accent()
                    .format(|v| format!("{v:.0}"))
                    .with_field(window, cx)
            }),
            delay: cx.new(|cx| {
                Slider::new("Audio track delay", SliderModel::new(-10., 10., 0.05, 0.), window, cx)
                    .accent()
                    .centered()
                    .format(|v| format!("{v:.3} s"))
                    .with_field(window, cx)
            }),
            eq: (0..EQ_GAINS.len()).map(|band| cx.new(|cx| eq_band(band, window, cx))).collect(),
        }
    }
}

fn shared(items: &[&'static str]) -> Vec<SharedString> {
    items.iter().copied().map(SharedString::from).collect()
}

fn saturation(window: &mut Window, cx: &mut Context<Slider>) -> Slider {
    Slider::new("Saturation", SliderModel::new(0., 3., 0.01, 1.), window, cx)
        .format(|v| format!("{v:.2}"))
}

fn eq_band(band: usize, window: &mut Window, cx: &mut Context<Slider>) -> Slider {
    let mut model = SliderModel::new(-20., 20., 0.5, 0.);
    model.set(EQ_GAINS[band]);
    Slider::new(format!("Band {}", band + 1), model, window, cx)
        .vertical()
        .centered()
        .accent()
        .format(|v| format!("{v:+.1} dB"))
}

fn media_grid(window: &mut Window, cx: &mut Context<VirtualList>) -> VirtualList {
    VirtualList::new(
        "Media",
        Shape { len: MEDIA_ITEMS, columns: Columns::Fit { width: 120., gap: 8. }, row_height: 96. },
        media_card,
        window,
        cx,
    )
}

/// One item of the media grid: a thumbnail well and a file name.
fn media_card(
    index: usize,
    state: list::ItemState,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = theme::palette(window, cx);
    div()
        .size_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(4.))
        .rounded(px(6.))
        .border_2()
        .border_color(card_border(state, &p))
        .when(state.selected, |c| c.bg(p.selected))
        .hover(|s| s.bg(p.hover))
        .child(div().flex_1().rounded(px(4.)).bg(p.well))
        .child(
            div()
                .text_size(px(12.))
                .text_color(p.text)
                .truncate()
                .child(SharedString::from(format!("Video {:05}.mkv", index + 1))),
        )
        .into_any_element()
}

/// A focus ring on the selected card while the grid has focus, a plain highlight otherwise.
fn card_border(state: list::ItemState, p: &Palette) -> Hsla {
    if state.selected && state.focused {
        p.focus_ring
    } else if state.selected {
        p.selected
    } else {
        gpui::transparent_black()
    }
}
