//! The miniplayer's hover controls (S4.10.3, S8.5.1; `miniplayer-variant3`, `-variant5`),
//! shared by full library mode's card and the Picture in Picture window: ✕ at the top left, a
//! window button at the top right, and Previous, Play/Pause and Next in the middle, over a dark
//! scrim. They show while the picture is hovered, and stay once the media ended, with Replay in
//! the middle.

use std::rc::Rc;

use gpui::{App, ClickEvent, Div, SharedString, Window, div, prelude::*, px};

use crate::assets::icons;
use crate::ui::button::IconButton;
use crate::ui::metrics;

/// The corner buttons' size and icons, and the transport's.
const CORNER_BUTTON: f32 = 32.;
const CORNER_ICON: f32 = 20.;
const TRANSPORT_BUTTON: f32 = 44.;
const TRANSPORT_ICON: f32 = 28.;
const TRANSPORT_GAP: f32 = 40.;
/// The space around the corner buttons.
const CORNER_PAD: f32 = 8.;
/// The scrim over the picture while the controls show.
const SCRIM: f32 = 0.55;

/// What a click on one of the controls does.
pub type OnClick = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// What the transport's middle button does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Middle {
    /// Plays: playback is paused or stopped.
    #[default]
    Play,
    /// Pauses: playback runs, or was asked to.
    Pause,
    /// Plays again: the media played to its end, and the controls stay up.
    Replay,
}

/// What the transport can do now.
#[derive(Clone, Copy, Debug, Default)]
pub struct TransportState {
    /// The middle button.
    pub middle: Middle,
    /// Previous would do something.
    pub can_go_back: bool,
    /// Next would do something.
    pub can_go_next: bool,
}

/// The top-right button: what it is called and shows.
pub struct WindowButton {
    /// Its icon.
    pub icon: &'static str,
    /// Its tooltip.
    pub label: SharedString,
}

/// What each control does.
pub struct Handlers {
    /// ✕: stops playback.
    pub close: OnClick,
    /// The top-right button.
    pub window: OnClick,
    /// Previous.
    pub previous: OnClick,
    /// Play/Pause, or Replay once ended.
    pub play: OnClick,
    /// Next.
    pub next: OnClick,
}

/// The controls over a picture in hover `group`; `in_title_bar` keeps a press on a button from
/// moving a window the picture drags.
pub fn overlay(
    group: &'static str,
    state: TransportState,
    button: WindowButton,
    handlers: &Handlers,
    in_title_bar: bool,
) -> Div {
    let corner = |id: &'static str, glyph: &'static str, label: SharedString, click: OnClick| {
        let button = IconButton::new(id, glyph, label)
            .size(CORNER_BUTTON, CORNER_ICON)
            .color(gpui::white())
            .on_click(move |event, window, cx| click(event, window, cx));
        if in_title_bar { button.in_title_bar() } else { button }
    };
    let close = corner("overlay-close", icons::CLOSE, "Stop".into(), handlers.close.clone());
    let window = corner("overlay-window", button.icon, button.label, handlers.window.clone());
    let corners = div().flex().justify_between().p(px(CORNER_PAD)).child(close).child(window);
    let overlay = div()
        .absolute()
        .inset_0()
        .flex()
        .flex_col()
        .rounded(px(metrics::RADIUS))
        .bg(gpui::black().opacity(SCRIM))
        .child(corners)
        .child(transport(state, handlers, in_title_bar));
    if state.middle == Middle::Replay {
        overlay
    } else {
        overlay.opacity(0.).group_hover(group, |s| s.opacity(1.))
    }
}

/// Previous, Play/Pause (Replay once ended) and Next.
fn transport(state: TransportState, handlers: &Handlers, in_title_bar: bool) -> Div {
    let (middle, label) = match state.middle {
        Middle::Replay => (icons::REPLAY, "Replay"),
        Middle::Pause => (icons::PAUSE, "Pause"),
        Middle::Play => (icons::PLAY, "Play"),
    };
    let button = |id: &'static str, glyph: &'static str, label: &'static str, click: &OnClick| {
        let click = click.clone();
        let button = IconButton::new(id, glyph, label)
            .size(TRANSPORT_BUTTON, TRANSPORT_ICON)
            .color(gpui::white())
            .on_click(move |event, window, cx| click(event, window, cx));
        if in_title_bar { button.in_title_bar() } else { button }
    };
    let previous = button("overlay-previous", icons::PREVIOUS, "Previous", &handlers.previous)
        .disabled(!state.can_go_back);
    let next =
        button("overlay-next", icons::NEXT, "Next", &handlers.next).disabled(!state.can_go_next);
    div()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(TRANSPORT_GAP))
        .child(previous)
        .child(button("overlay-play", middle, label, &handlers.play))
        .child(next)
}
