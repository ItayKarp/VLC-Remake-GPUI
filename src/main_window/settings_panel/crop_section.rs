//! The Video tab's Crop section (S7.2.5, `panels/video/crop/*`): **Top**, **Bottom**, **Left** and
//! **Right** in pixels, two to a row, each row with a link toggle. It folds and has a title,
//! Reset and an enable toggle like the effect sections (D23).
//!
//! **Linked edges** aren't designed: the toggle beside a pair shows the two halves of a chain
//! apart while unlinked (as in the mock) and joined in the accent while linked; linked, the pair
//! moves together, and linking makes the second edge match the first. Links last the session,
//! like the values.
//!
//! The pixels are the video's own, as its tooltip says, and at least [`MIN_KEPT`] stay each way:
//! a value that would leave less, for the video playing, turns its field red with the most it
//! may be, and changes nothing.

use gpui::{App, AppContext as _, Context, Div, Entity, Subscription, Window, div, prelude::*, px};

use super::effect_section::{self, BOX_PADDING};
use crate::assets::icons;
use crate::effects::{self, VIDEO_CROP};
use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::settings::registry::Value;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;
use crate::ui::text_input::{self, Look, TextInput, Validation};
use crate::ui::{metrics, tooltip};
use crate::video::framing::{CropEdges, MIN_KEPT};

/// The fields' height and text size, the space between them, and the link button's size.
const FIELD_HEIGHT: f32 = 32.;
const TEXT: f32 = 14.;
const GAP: f32 = 8.;
const LINK_SIZE: f32 = 28.;
/// What the fields count.
const PIXELS_TIP: &str = tr_noop!("In the video's own pixels, not the screen's");

/// An edge: which side, and how its field and its pair's are found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    const ALL: [Self; 4] = [Self::Top, Self::Bottom, Self::Left, Self::Right];

    fn key(self) -> &'static str {
        VIDEO_CROP.controls[self as usize]
    }

    fn label(self) -> &'static str {
        match self {
            Self::Top => tr!("Top"),
            Self::Bottom => tr!("Bottom"),
            Self::Left => tr!("Left"),
            Self::Right => tr!("Right"),
        }
    }

    /// The opposite edge, which it shares a link with.
    fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }

    /// Which link it is under: 0 for top and bottom, 1 for left and right.
    fn pair(self) -> usize {
        self as usize / 2
    }

    /// Its value in `edges`.
    fn of(self, edges: &CropEdges) -> u32 {
        match self {
            Self::Top => edges.top,
            Self::Bottom => edges.bottom,
            Self::Left => edges.left,
            Self::Right => edges.right,
        }
    }
}

/// The Crop section's fields and links.
pub(super) struct CropControls {
    fields: [Entity<TextInput>; 4],
    /// Whether top and bottom, and left and right, move together.
    linked: [bool; 2],
    _events: Vec<Subscription>,
}

impl CropControls {
    /// The fields, changing the session when a value is entered.
    pub(super) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let fields = Edge::ALL.map(|edge| {
            cx.new(|cx| TextInput::new(window, cx).with_look(Look::Bare).with_label(edge.label()))
        });
        let events = Edge::ALL
            .iter()
            .zip(&fields)
            .map(|(&edge, field)| {
                cx.subscribe(field, move |this, _, event, cx| this.on_crop_field(edge, event, cx))
            })
            .collect();
        Self { fields, linked: [false; 2], _events: events }
    }

    /// Shows the values in effect in the fields not being typed in.
    pub(super) fn sync(&self, window: &Window, cx: &mut Context<MainWindow>) {
        let edges = effects::effective(cx).prefs.video.crop_edges;
        for (edge, field) in Edge::ALL.iter().zip(&self.fields) {
            let text = edge.of(&edges).to_string();
            if !field.read(cx).is_focused(window) && field.read(cx).text() != text {
                field.update(cx, |field, cx| show(field, text, cx));
            }
        }
    }

    fn field(&self, edge: Edge) -> &Entity<TextInput> {
        &self.fields[edge as usize]
    }
}

impl MainWindow {
    /// The Crop section: its title row over the folding box of fields.
    pub(super) fn render_crop_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let content = effect_section::is_expanded(cx, &VIDEO_CROP).then(|| {
            let controls = &self.settings_panel.crop;
            let error = Edge::ALL
                .iter()
                .find_map(|edge| invalid_reason(controls.field(*edge), cx))
                .map(|why| div().text_size(text_px(TEXT)).text_color(p.danger).child(why));
            div()
                .flex_row_start()
                .flex_col()
                .gap(px(GAP))
                .px(px(BOX_PADDING))
                .pb(px(BOX_PADDING))
                .child(self.edge_row([Edge::Top, Edge::Bottom], p, cx))
                .child(self.edge_row([Edge::Left, Edge::Right], p, cx))
                .children(error)
        });
        effect_section::folding(&VIDEO_CROP, content, p, cx)
    }

    /// Two opposite edges' fields and their link.
    fn edge_row(&self, edges: [Edge; 2], p: &Palette, cx: &mut Context<Self>) -> Div {
        let pair = edges[0].pair();
        let linked = self.settings_panel.crop.linked[pair];
        let (first, second) = (edges[0].label(), edges[1].label());
        let (icon, name) = if linked {
            (icons::LINK, tr!("Unlink {first} and {second}", first, second))
        } else {
            (icons::LINK_OFF, tr!("Link {first} and {second}", first, second))
        };
        let link = IconButton::new(format!("crop-link-{pair}"), icon, name)
            .size(LINK_SIZE, metrics::ICON)
            .selected(linked)
            .when(linked, |button| button.color(p.accent))
            .on_click(cx.listener(move |this, _, _, cx| this.toggle_crop_link(edges[0], cx)));
        div()
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .children(edges.map(|edge| self.edge_field(edge, p)))
            .child(link)
    }

    /// One edge's box: its label, its field and "px".
    fn edge_field(&self, edge: Edge, p: &Palette) -> impl IntoElement {
        div()
            .id(("crop-edge", edge as usize))
            .flex_1()
            .min_w_0()
            .h(px(FIELD_HEIGHT))
            .flex_row_start()
            .items_center()
            .gap(px(GAP))
            .px(px(GAP + GAP))
            .rounded(px(metrics::RADIUS))
            .bg(p.well)
            .text_size(text_px(TEXT))
            .tooltip(tooltip::text(crate::i18n::text(PIXELS_TIP), None::<&str>))
            .child(div().flex_none().text_color(p.text_muted).child(edge.label()))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(p.text)
                    .child(self.settings_panel.crop.field(edge).clone()),
            )
            .child(div().flex_none().text_color(p.text_muted).child("px"))
    }

    /// Links or unlinks `edge` and its opposite; linking gives the opposite edge its value.
    fn toggle_crop_link(&mut self, edge: Edge, cx: &mut Context<Self>) {
        let linked = &mut self.settings_panel.crop.linked[edge.pair()];
        *linked = !*linked;
        if *linked {
            let value = edge.of(&effects::effective(cx).prefs.video.crop_edges);
            effects::change(cx, &VIDEO_CROP, edge.opposite().key(), Value::Number(value.into()));
        }
        cx.notify();
    }

    fn on_crop_field(&mut self, edge: Edge, event: &text_input::Event, cx: &mut Context<Self>) {
        let field = self.settings_panel.crop.field(edge).clone();
        match event {
            text_input::Event::Changed => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            text_input::Event::Submitted | text_input::Event::Blurred => {
                let typed = field.read(cx).text().to_owned();
                match self.crop_value(edge, &typed, cx) {
                    Ok(value) => self.set_crop_edge(edge, value, cx),
                    Err(why) => field.update(cx, |field, cx| {
                        field.set_validation(Validation::Error(why.into()), cx);
                    }),
                }
                cx.notify();
            }
        }
    }

    /// The pixels typed for `edge`, or why they can't be.
    fn crop_value(&self, edge: Edge, typed: &str, cx: &App) -> Result<u32, String> {
        let value =
            parse_pixels(typed).ok_or_else(|| tr!("A whole number of pixels").to_owned())?;
        let Some(side) = self.crop_side(edge, cx) else { return Ok(value) };
        let linked = self.settings_panel.crop.linked[edge.pair()];
        let other = edge.opposite().of(&effects::effective(cx).prefs.video.crop_edges);
        let most = most_for(CropEdges::most(side), other, linked);
        if value <= most {
            return Ok(value);
        }
        Err(tr!("At most {most} px, so {kept} px of the picture stay", most, kept = MIN_KEPT))
    }

    /// The playing video's pixels along `edge`'s axis, when known.
    fn crop_side(&self, edge: Edge, cx: &App) -> Option<u32> {
        let (player, _) = self.player.as_ref()?;
        let size = player.read(cx).video_size()?;
        Some(if edge.pair() == 0 { size.height } else { size.width })
    }

    /// Sets `edge`, and its opposite while linked, for the session.
    fn set_crop_edge(&mut self, edge: Edge, value: u32, cx: &mut Context<Self>) {
        effects::change(cx, &VIDEO_CROP, edge.key(), Value::Number(value.into()));
        if self.settings_panel.crop.linked[edge.pair()] {
            effects::change(cx, &VIDEO_CROP, edge.opposite().key(), Value::Number(value.into()));
        }
    }
}

/// Shows `text` in `field`, clearing any error.
fn show(field: &mut TextInput, text: String, cx: &mut Context<TextInput>) {
    field.set_text(text, cx);
    field.set_validation(Validation::None, cx);
}

/// Why a field's value was refused, if it was.
fn invalid_reason(field: &Entity<TextInput>, cx: &App) -> Option<String> {
    match field.read(cx).validation() {
        Validation::Error(why) => Some(why.to_string()),
        Validation::None => None,
    }
}

/// The most one edge may trim when two opposite ones may trim `most` together: all that the
/// other leaves, or half while they move together.
fn most_for(most: u32, other: u32, linked: bool) -> u32 {
    if linked { most / 2 } else { most.saturating_sub(other) }
}

/// Typed pixels: "12", "12 px"; empty is none.
fn parse_pixels(text: &str) -> Option<u32> {
    let text = text.trim().to_lowercase();
    let number = text.strip_suffix("px").unwrap_or(&text).trim();
    if number.is_empty() {
        return Some(0);
    }
    number.parse().ok()
}

#[cfg(test)]
mod tests;
