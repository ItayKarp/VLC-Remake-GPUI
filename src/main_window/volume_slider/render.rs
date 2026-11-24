//! Drawing the volume control: the speaker with its level icon (S2.5.4), the wedge, the value
//! bubble (`volume/audio_value.png`, S2.5.3) and the slider's accessible description.
//!
//! Muting isn't designed: the speaker is struck through and the fill fades to a third, so the
//! level that unmuting restores stays visible.

use gpui::{
    AnyElement, App, ClickEvent, Context, CursorStyle, Div, Entity, MouseButton, MouseMoveEvent,
    MouseUpEvent, ScrollEvent, Stateful, Window, accesskit, canvas, div, prelude::*, px,
};

use super::percent_text;
use super::scale::Scale;
use super::wedge::{self, Look, WEDGE_HEIGHT, WEDGE_WIDTH};
use crate::assets::icons;
use crate::engine::player::Player;
use crate::engine::values::Volume;
use crate::main_window::MainWindow;
use crate::main_window::control_bar::{BUTTON, ICON, LEVEL_GAP};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;
use crate::ui::{focus, tooltip};

/// The wedge's tooltip without an audio device.
const NO_DEVICE: &str = "No audio device: the volume can't change";
/// The focus ring's width around the wedge.
const RING: f32 = 2.;
/// The value bubble's size and corner radius, from the design.
const BUBBLE_WIDTH: f32 = 121.;
const BUBBLE_HEIGHT: f32 = 37.;
const BUBBLE_RADIUS: f32 = 6.;
/// The bubble's text size.
const BUBBLE_TEXT: f32 = 15.;
/// The space between the bubble and the wedge.
const BUBBLE_GAP: f32 = 8.;
/// How opaque the fill is while muted or without an audio device.
const DIMMED: f32 = 0.35;
/// The levels below which the speaker shows fewer waves.
const LOW_BELOW: f32 = 0.34;
const MEDIUM_BELOW: f32 = 0.67;

/// Whether the volume can change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Availability {
    /// Nothing is loaded.
    NoMedia,
    /// The machine has no audio device (S1.5).
    NoDevice,
    /// It can.
    Ready,
}

/// What the control shows, read from the player once per frame.
#[derive(Clone, Copy, Debug)]
struct Shown {
    volume: Volume,
    max: Volume,
    muted: bool,
    availability: Availability,
}

impl Shown {
    fn of(player: Option<&Entity<Player>>, cx: &App) -> Self {
        let Some(player) = player.map(|player| player.read(cx)) else {
            let (volume, max) = (Volume::new(0.), Volume::FULL);
            return Self { volume, max, muted: false, availability: Availability::NoMedia };
        };
        let availability =
            if player.has_no_audio_device() { Availability::NoDevice } else { Availability::Ready };
        Self {
            volume: player.volume(),
            max: player.max_volume(),
            muted: player.is_muted(),
            availability,
        }
    }

    fn usable(self) -> bool {
        self.availability == Availability::Ready
    }
}

impl MainWindow {
    /// The speaker, which mutes, and the wedge.
    pub(in crate::main_window) fn render_volume(
        &self,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let player = self.loaded_player(cx);
        let shown = Shown::of(player.as_ref(), cx);
        let label = if shown.muted { "Unmute" } else { "Mute" };
        let speaker = IconButton::new("speaker", speaker_icon(shown), label)
            .size(BUTTON, ICON)
            .disabled(player.is_none())
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(player) = this.loaded_player(cx) {
                    player.update(cx, Player::toggle_mute);
                }
            }));
        div()
            .flex()
            .flex_row()
            .flex_none()
            .items_center()
            .gap(px(LEVEL_GAP))
            .child(speaker)
            .child(self.render_wedge(p, shown, cx))
            .into_any_element()
    }

    fn render_wedge(&self, p: &Palette, shown: Shown, cx: &mut Context<Self>) -> Stateful<Div> {
        let slider = &self.volume_slider;
        let bubble = (slider.dragging || slider.bubble.is_showing()).then(|| bubble(p, shown));
        let wedge = div()
            .id("volume")
            .relative()
            .flex_none()
            .w(px(WEDGE_WIDTH + 2. * RING))
            .h(px(WEDGE_HEIGHT + 2. * RING))
            .rounded(px(4.))
            .border(px(RING))
            .border_color(gpui::transparent_black())
            .key_context(focus::CONTROL)
            .track_focus(&slider.focus)
            .cursor(if shown.usable() {
                CursorStyle::PointingHand
            } else {
                CursorStyle::OperationNotAllowed
            })
            .on_key_down(cx.listener(Self::volume_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::start_volume_drag))
            .on_scroll(cx.listener(|this, event: &ScrollEvent, window, cx| {
                this.scroll_volume(event, false, window, cx);
            }))
            .when(shown.availability == Availability::NoDevice, |wedge| {
                wedge.tooltip(tooltip::text(NO_DEVICE, None::<&str>))
            })
            .child(slider.track.element())
            .child(painted(p, shown))
            .children(bubble)
            .children(self.volume_drag_listener(cx));
        accessible(focus::ring(wedge, p), shown, cx)
    }

    /// While dragging, follows the pointer anywhere in the window (and past its edges).
    fn volume_drag_listener(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.volume_slider.dragging {
            return None;
        }
        let entity = cx.entity();
        let listener =
            canvas(|_, _, _| {}, move |_, (), window, _| listen_for_drag(entity, window));
        Some(listener.absolute().size_0().into_any_element())
    }
}

/// The wedge itself, painted from the level.
fn painted(p: &Palette, shown: Shown) -> impl IntoElement {
    let look = Look {
        track: p.well,
        segments: Scale::new(shown.max).segments(shown.volume),
        opacity: if shown.muted || !shown.usable() { DIMMED } else { 1. },
    };
    canvas(|_, _, _| {}, move |bounds, (), window, _| wedge::paint(bounds, &look, window))
        .size_full()
}

/// The speaker for the level: struck through while muted, then none, one, two or three waves.
fn speaker_icon(shown: Shown) -> &'static str {
    let level = shown.volume.get();
    if shown.muted {
        icons::VOLUME_MUTED
    } else if shown.volume.as_percent() == 0 {
        icons::VOLUME_NONE
    } else if level < LOW_BELOW {
        icons::VOLUME_LOW
    } else if level < MEDIUM_BELOW {
        icons::VOLUME_MEDIUM
    } else {
        icons::VOLUME
    }
}

/// The value bubble, centred over the wedge.
fn bubble(p: &Palette, shown: Shown) -> Div {
    let text = percent_text(shown.volume);
    div()
        .absolute()
        .bottom(px(WEDGE_HEIGHT + RING + BUBBLE_GAP))
        .left(px((WEDGE_WIDTH - BUBBLE_WIDTH) / 2.))
        .w(px(BUBBLE_WIDTH))
        .h(px(BUBBLE_HEIGHT))
        .rounded(px(BUBBLE_RADIUS))
        .bg(p.panel)
        .shadow_lg()
        .flex()
        .items_center()
        .justify_center()
        .text_size(text_px(BUBBLE_TEXT))
        .text_color(p.text)
        .child(text)
}

/// Describes the wedge to screen readers as a slider over percents.
fn accessible(wedge: Stateful<Div>, shown: Shown, cx: &App) -> Stateful<Div> {
    let step = crate::main_window::audio::volume_step(crate::settings::get(cx));
    let value = percent_text(shown.volume);
    let value = if shown.muted { format!("Muted, {value}") } else { value };
    wedge
        .role(accesskit::Role::Slider)
        .aria_label("Volume")
        .aria_numeric_value(f64::from(shown.volume.as_percent()))
        .aria_min_numeric_value(0.)
        .aria_max_numeric_value(f64::from(shown.max.as_percent()))
        .aria_numeric_value_step(f64::from(step.as_percent()))
        .aria_value(value)
        .aria_orientation(accesskit::Orientation::Horizontal)
}

/// Sends every pointer move and the release to the window, wherever the pointer is.
fn listen_for_drag(main: Entity<MainWindow>, window: &mut Window) {
    let moved = main.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
        if phase.bubble() && event.dragging() {
            moved.update(cx, |this, cx| this.drag_volume_to(event.position, cx));
        }
    });
    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
        if phase.bubble() {
            main.update(cx, MainWindow::end_volume_drag);
        }
    });
}
