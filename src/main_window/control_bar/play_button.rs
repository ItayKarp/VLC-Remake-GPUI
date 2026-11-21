//! The Play/Pause pill (S2.2.2, `play_button/*`): ▶ on the accent while paused or stopped, ❚❚
//! on grey while playing, and a spinner on grey while opening or buffering (not designed).

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, ClickEvent, Hsla, SharedString, Transformation,
    Window, div, percentage, prelude::*, px, svg,
};

use super::layout::{PILL_HEIGHT, PILL_WIDTH};
use crate::assets::icons;
use crate::engine::state::PlaybackState;
use crate::theme::{self, Palette, blend};
use crate::ui::{focus, metrics, tooltip};

/// The glyph's size inside the pill.
const GLYPH: f32 = 18.;
/// How long the spinner takes to turn once.
const SPIN: Duration = Duration::from_millis(900);

/// Called when the pill is clicked or activated from the keyboard.
type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// What the pill shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    /// ▶: pressing plays.
    Play,
    /// ❚❚: pressing pauses.
    Pause,
    /// A spinner: the media is opening or buffering and will play; pressing pauses.
    Waiting,
}

impl Face {
    /// The face for a player in `state` that the user wants playing (`wants_to_play`) or
    /// paused. It follows the request, so it changes on the click, not when the engine answers.
    pub fn of(state: PlaybackState, wants_to_play: bool) -> Self {
        match state {
            PlaybackState::Opening | PlaybackState::Buffering if wants_to_play => Face::Waiting,
            PlaybackState::Playing if wants_to_play => Face::Pause,
            _ => Face::Play,
        }
    }

    /// What pressing the pill does, for its tooltip and screen readers.
    pub fn label(self) -> &'static str {
        match self {
            Face::Play => "Play",
            Face::Pause | Face::Waiting => "Pause",
        }
    }
}

/// The Play/Pause pill.
#[derive(IntoElement)]
pub struct PlayButton {
    face: Face,
    shortcut: Option<SharedString>,
    on_click: ClickHandler,
}

impl PlayButton {
    /// A pill showing `face`, naming `shortcut` in its tooltip, that runs `on_click`.
    pub fn new(
        face: Face,
        shortcut: Option<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { face, shortcut, on_click: Rc::new(on_click) }
    }
}

/// The pill's colors: its face at rest, hovered and pressed, and the glyph.
struct Colors {
    rest: Hsla,
    hover: Hsla,
    pressed: Hsla,
    glyph: Hsla,
}

fn colors(face: Face, p: &Palette) -> Colors {
    if face == Face::Play {
        return Colors {
            rest: p.accent,
            hover: blend(p.accent, p.background, 0.85),
            pressed: blend(p.accent, p.background, 0.7),
            glyph: p.on_accent,
        };
    }
    let grey = |amount| blend(p.icon, p.control_bar, amount);
    let glyph = if p.dark { p.sunken } else { gpui::white() };
    Colors { rest: grey(0.5), hover: grey(0.65), pressed: grey(0.35), glyph }
}

impl RenderOnce for PlayButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let Colors { rest, hover, pressed, glyph } = colors(self.face, &p);
        let label = self.face.label();
        let on_click = self.on_click;
        let pill = div()
            .id("play-pause")
            .w(px(PILL_WIDTH))
            .h(px(PILL_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(metrics::RADIUS))
            .border_2()
            .border_color(gpui::transparent_black())
            .bg(rest)
            .tab_index(0)
            .cursor_pointer()
            .occlude()
            .hover(move |style| style.bg(hover))
            .active(move |style| style.bg(pressed))
            .aria_label(tooltip::describe(label, self.shortcut.as_deref()))
            .tooltip(tooltip::text(label, self.shortcut))
            .on_click(move |event, window, cx| on_click(event, window, cx))
            .child(self::glyph(self.face, glyph));
        focus::ring(pill, &p)
    }
}

fn glyph(face: Face, color: Hsla) -> AnyElement {
    let icon = |path| svg().path(path).size(px(GLYPH)).flex_none().text_color(color);
    match face {
        Face::Play => icon(icons::PLAY).into_any_element(),
        Face::Pause => icon(icons::PAUSE).into_any_element(),
        Face::Waiting => icon(icons::SPINNER)
            .with_animation("buffering", Animation::new(SPIN).repeat(), |spinner, turned| {
                spinner.with_transformation(Transformation::rotate(percentage(turned)))
            })
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::state::ErrorKind;

    #[test]
    fn shows_play_unless_playing_or_about_to() {
        use PlaybackState::{Ended, Error, Idle, Paused, Stopped};
        for state in [Idle, Paused, Ended, Stopped, Error(ErrorKind::Corrupt)] {
            assert_eq!(Face::of(state, true), Face::Play, "{state:?}");
        }
        assert_eq!(Face::of(PlaybackState::Playing, true), Face::Pause);
    }

    #[test]
    fn spins_while_waiting_to_play() {
        assert_eq!(Face::of(PlaybackState::Buffering, true), Face::Waiting);
        assert_eq!(Face::of(PlaybackState::Opening, true), Face::Waiting);
    }

    #[test]
    fn a_pause_shows_at_once() {
        for state in [PlaybackState::Playing, PlaybackState::Buffering] {
            assert_eq!(Face::of(state, false), Face::Play, "{state:?}");
        }
    }
}
