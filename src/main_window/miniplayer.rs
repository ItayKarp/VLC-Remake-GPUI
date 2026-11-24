//! Full library mode's miniplayer (S4.10.3–S4.10.5, `miniplayer/*`): a card docked in a corner of
//! the library while something plays, or has just ended ([`card`]), with the queue expanding from
//! it ([`queue`]). Nothing playing, no card (`player_06`).
//!
//! Dragging the card moves it, and letting go docks it in the corner of the quarter it is in,
//! remembered across launches; a drag only starts past a few pixels, so a click on its buttons
//! stays a click. The card shrinks in a narrow library and hides when it would be too small
//! ([`dock`]). The listing leaves room under its last line for it.

mod card;

pub(super) use card::progress;
mod dock;
mod queue;

use gpui::{
    AnyElement, App, Context, DispatchPhase, Entity, FocusHandle, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Window, canvas, div, prelude::*,
    px,
};

use self::dock::{Area, CardSize, MARGIN};
use super::{MainWindow, TITLE_BAR_HEIGHT};
use crate::engine::player::Player;
use crate::engine::state::PlaybackState;
use crate::settings::{self, MiniplayerCorner};
use crate::theme::Palette;
use crate::ui::metrics;
use crate::ui::popover::BoundsTracker;

/// How far the pointer must move with the button down before the card follows it.
const DRAG_START: f32 = 4.;
/// The queue's height above the card (`miniplayer-variant2`), and the least it gets before it
/// doesn't show.
const QUEUE_HEIGHT: f32 = 360.;
const MIN_QUEUE_HEIGHT: f32 = 140.;

/// The miniplayer's state: its queue, its drag and where it was last placed.
#[derive(Default)]
pub(super) struct Miniplayer {
    /// The queue shows above the card.
    queue_open: bool,
    drag: Option<CardDrag>,
    /// Where the card and its queue were placed this frame, while they show.
    placed: Option<Placed>,
    /// The progress bar's bounds, for seeking where it is clicked.
    progress: BoundsTracker,
    /// Holds the hover controls, to tell when Tab reached one of them; made on first use.
    overlay_focus: Option<FocusHandle>,
    /// Whether one of the hover controls had the keyboard focus this frame.
    overlay_focused: bool,
}

/// A press on the card that may become a drag.
#[derive(Clone, Copy)]
struct CardDrag {
    /// Where the pointer is, relative to the card's top-left corner when pressed.
    grab: (f32, f32),
    /// Where the press was, and where the pointer is now.
    start: Point<Pixels>,
    pointer: Point<Pixels>,
    /// The pointer went past [`DRAG_START`]: the card follows it.
    moving: bool,
}

/// Where the card shows this frame.
#[derive(Clone, Copy)]
struct Placed {
    area: Area,
    card: CardSize,
    /// The queue's height, while it shows.
    queue: f32,
    /// The top-left corner of the card and its queue.
    origin: (f32, f32),
}

impl Placed {
    /// The card and its queue's size.
    fn size(self) -> (f32, f32) {
        (self.card.width, self.card.height() + self.queue)
    }
}

impl MainWindow {
    /// The player whose media the card shows: in full library mode, while something plays or
    /// has just ended.
    fn miniplayer_player(&self, cx: &App) -> Option<Entity<Player>> {
        if !self.library_panel.is_full() || self.windowing.pip.is_shown() {
            return None;
        }
        let player = self.loaded_player(cx)?;
        let state = player.read(cx).state();
        (state.is_active() || state == PlaybackState::Ended).then_some(player)
    }

    /// Places the card for this frame, in the library right of its navigation and under the
    /// title bar when `title_bar` shows.
    pub(super) fn place_miniplayer(&mut self, title_bar: bool, window: &Window, cx: &App) {
        let size = window.viewport_size();
        let top = if title_bar { TITLE_BAR_HEIGHT } else { 0. };
        let left = Self::library_nav_space();
        let area = Area {
            left,
            top,
            width: f32::from(size.width) - left,
            height: f32::from(size.height) - top,
        };
        let placed = self.miniplayer_player(cx).and_then(|_| CardSize::fit(area)).map(|card| {
            let room = area.height - card.height() - 2. * MARGIN;
            let queue = QUEUE_HEIGHT.min(room);
            let queue =
                if self.miniplayer.queue_open && queue >= MIN_QUEUE_HEIGHT { queue } else { 0. };
            let mut placed = Placed { area, card, queue, origin: (0., 0.) };
            placed.origin = self.card_origin(placed, cx);
            placed
        });
        self.miniplayer.placed = placed;
        if placed.is_none() {
            self.miniplayer.drag = None;
        }
    }

    /// Where the card goes: under the pointer while dragged, else in its corner.
    fn card_origin(&self, placed: Placed, cx: &App) -> (f32, f32) {
        match self.miniplayer.drag {
            Some(drag) if drag.moving => {
                let (x, y) = (f32::from(drag.pointer.x), f32::from(drag.pointer.y));
                dock::clamp(placed.area, placed.size(), (x - drag.grab.0, y - drag.grab.1))
            }
            _ => dock::origin(corner(cx), placed.area, placed.size()),
        }
    }

    /// How much room the listing leaves under its last line, so the card hides nothing.
    pub(super) fn miniplayer_clearance(&self) -> f32 {
        self.miniplayer.placed.map_or(0., |placed| placed.size().1 + MARGIN)
    }

    /// The card, and its queue while open, where they were placed.
    pub(super) fn render_miniplayer(
        &mut self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let placed = self.miniplayer.placed?;
        let player = self.miniplayer_player(cx)?;
        let overlay_focus = self.miniplayer.overlay_focus.get_or_insert_with(|| cx.focus_handle());
        self.miniplayer.overlay_focused =
            overlay_focus.contains_focused(window, cx) && window.last_input_was_keyboard();
        let (width, height) = placed.size();
        let (x, y) = placed.origin;
        let card = self.render_miniplayer_card(&player, placed.card, p, cx);
        let queue = (placed.queue > 0.).then(|| self.render_miniplayer_queue(placed.queue, p, cx));
        let below = matches!(corner(cx), MiniplayerCorner::TopLeft | MiniplayerCorner::TopRight);
        let container = div()
            .id("miniplayer")
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(width))
            .h(px(height))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(metrics::RADIUS_LARGE))
            .bg(p.panel)
            .text_color(p.text)
            .shadow_lg()
            .occlude()
            .on_mouse_down(MouseButton::Left, cx.listener(Self::press_card));
        let container = if below {
            container.child(card).children(queue)
        } else {
            container.children(queue).child(card)
        };
        Some(div().child(container).child(Self::drag_watcher(cx)).into_any_element())
    }

    /// Whether the queue shows above the card.
    pub(super) fn is_miniplayer_queue_open(&self) -> bool {
        self.miniplayer.queue_open
    }

    /// The queue toggle: shows or hides the queue above the card.
    pub(super) fn toggle_miniplayer_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.miniplayer.queue_open = !self.miniplayer.queue_open;
        if self.miniplayer.queue_open {
            self.focus_docked_queue(window, cx);
        } else {
            self.blur_docked_queue(window, cx);
        }
        cx.notify();
    }

    fn press_card(&mut self, event: &MouseDownEvent, _: &mut Window, _: &mut Context<Self>) {
        let Some(placed) = self.miniplayer.placed else { return };
        let (x, y) = placed.origin;
        let grab = (f32::from(event.position.x) - x, f32::from(event.position.y) - y);
        let start = event.position;
        self.miniplayer.drag = Some(CardDrag { grab, start, pointer: start, moving: false });
    }

    /// Paints nothing; while the card is pressed, hears the pointer anywhere in the window.
    fn drag_watcher(cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        canvas(|_, _, _| {}, move |_, (), window, _| Self::follow_card_drag(&this, window))
            .absolute()
            .size_full()
    }

    fn follow_card_drag(this: &Entity<Self>, window: &mut Window) {
        let moves = this.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble {
                moves.update(cx, |this, cx| this.drag_card(event.position, cx));
            }
        });
        let ups = this.clone();
        // In the capture phase, so the button a dragged card is let go over doesn't click.
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Capture && ups.update(cx, Self::drop_card) {
                cx.stop_propagation();
            }
        });
    }

    fn drag_card(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(drag) = &mut self.miniplayer.drag else { return };
        drag.pointer = position;
        let moved = f32::from((position.x - drag.start.x).abs())
            .hypot(f32::from((position.y - drag.start.y).abs()));
        if drag.moving || moved >= DRAG_START {
            drag.moving = true;
            cx.notify();
        }
    }

    /// Ends a press; a drag docks the card in the corner it was let go nearest. Returns
    /// whether it was a drag.
    fn drop_card(&mut self, cx: &mut Context<Self>) -> bool {
        let drag = self.miniplayer.drag.take();
        let (Some(drag), Some(placed)) = (drag, self.miniplayer.placed) else { return false };
        if !drag.moving {
            return false;
        }
        let (width, height) = placed.size();
        let center = (placed.origin.0 + width / 2., placed.origin.1 + height / 2.);
        let corner = dock::nearest(placed.area, center);
        settings::update(cx, |s| s.local.miniplayer_corner = corner);
        cx.notify();
        true
    }
}

/// The corner the card docks in.
fn corner(cx: &App) -> MiniplayerCorner {
    settings::get(cx).local.miniplayer_corner
}
