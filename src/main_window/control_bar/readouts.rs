//! The time readouts (S2.4, `current_time/*`, `duration/*`).
//!
//! The current time shows a dark pill under the pointer; clicking it turns it into a field with
//! the time selected, bright digits and an orange caret. Enter jumps to what was typed
//! ([`time_entry`]); text that isn't a time turns red and nothing happens. Escape, or clicking
//! away, puts the readout back. The field keeps what was typed while playback goes on.
//!
//! Clicking the duration switches between the length and the time left (`-00:23:30`), for all
//! media and across launches (`prefs.interface.duration_readout`). The time left is media time,
//! as VLC shows it, whatever the playback rate. With the length unknown it shows `--:--:--` and
//! can't be clicked.

use gpui::{
    AnyElement, App, ClickEvent, Context, CursorStyle, Div, Entity, Focusable, KeyDownEvent,
    SharedString, Stateful, Subscription, Window, accesskit, div, prelude::*, px,
};

use super::clock;
use super::layout::TIME_WIDTH;
use super::time_entry;
use crate::engine::player::Player;
use crate::main_window::MainWindow;
use crate::settings;
use crate::theme::Palette;
use crate::ui::text_input::{self, Look, TextInput, Validation};

/// The readouts' text size.
const TEXT_SIZE: f32 = 13.;
/// The hover pill's and the field's height (`current_time-hover.png`).
const PILL_HEIGHT: f32 = 24.;
/// The pill's corner radius.
const PILL_RADIUS: f32 = 6.;
/// Room between the pill's edges and the digits.
const PILL_PADDING: f32 = 4.;
/// How strongly invalid input tints the field red.
const ERROR_TINT: f32 = 0.18;

/// The current time while it's being edited.
pub struct TimeEdit {
    input: Entity<TextInput>,
    _events: Subscription,
}

impl MainWindow {
    /// The current time: a readout to click, or the field while it's edited.
    pub(super) fn render_current_time(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if let Some(edit) = &self.control_bar.time_edit {
            return Self::render_time_field(edit, p, cx);
        }
        let player = self.loaded_player(cx).map(|player| player.read(cx));
        let text = clock::format(player.map(Player::position).unwrap_or_default());
        let seekable = player.is_some_and(|player| player.is_seekable() != Some(false));
        pill("current-time", p, text.clone())
            .aria_label(SharedString::from(format!("Current time {text}. Jump to a time")))
            .when(seekable, |pill| {
                clickable(pill, p).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.start_time_edit(window, cx);
                }))
            })
            .into_any_element()
    }

    /// The length or the time left, switched by a click.
    pub(super) fn render_duration(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let player = self.loaded_player(cx).map(|player| player.read(cx));
        let duration = player.and_then(Player::duration);
        let shown = settings::get(cx).prefs.interface.duration_readout;
        let text = match (duration, shown) {
            (None, _) => clock::UNKNOWN.to_owned(),
            (Some(duration), settings::DurationReadout::Total) => clock::format(duration),
            (Some(duration), settings::DurationReadout::Remaining) => {
                clock::format_remaining(player.map(Player::position).unwrap_or_default(), duration)
            }
        };
        let label = match shown {
            settings::DurationReadout::Total => format!("Length {text}. Show the time left"),
            settings::DurationReadout::Remaining => format!("Time left {text}. Show the length"),
        };
        pill("duration", p, text)
            .aria_label(SharedString::from(label))
            .when(duration.is_some(), |pill| {
                clickable(pill, p).on_click(|_: &ClickEvent, _, cx| toggle_duration_readout(cx))
            })
            .into_any_element()
    }

    fn render_time_field(edit: &TimeEdit, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let invalid = edit.input.read(cx).is_invalid();
        let background = if invalid { p.danger.opacity(ERROR_TINT) } else { p.sunken };
        frame("current-time-field")
            .px(px(PILL_PADDING))
            .bg(background)
            .text_color(p.text_strong)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                    this.end_time_edit(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(edit.input.clone())
            .into_any_element()
    }

    /// Turns the current time into a field holding it, selected.
    pub(in crate::main_window) fn start_time_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = self.loaded_player(cx).map(|player| player.read(cx).position());
        let text = clock::format(position.unwrap_or_default());
        let input = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_text(text)
                .with_all_selected()
                .with_look(Look::Bare)
                .with_label("Jump to time")
        });
        let events = cx.subscribe_in(&input, window, Self::on_time_input);
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        self.control_bar.time_edit = Some(TimeEdit { input, _events: events });
        cx.notify();
    }

    fn on_time_input(
        &mut self,
        input: &Entity<TextInput>,
        event: &text_input::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            text_input::Event::Changed => {
                input.update(cx, |input, cx| input.set_validation(Validation::None, cx));
            }
            text_input::Event::Blurred => self.end_time_edit(window, cx),
            text_input::Event::Submitted => match time_entry::parse(input.read(cx).text()) {
                Ok(target) => {
                    self.end_time_edit(window, cx);
                    self.seek_player(target, cx);
                }
                Err(time_entry::Invalid) => input.update(cx, |input, cx| {
                    input.set_validation(Validation::Error("Not a time".into()), cx);
                }),
            },
        }
    }

    /// Puts the readout back and gives the keyboard back to the player.
    fn end_time_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.control_bar.time_edit.take().is_some() {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }
}

/// Switches the duration readout between the length and the time left, for good.
fn toggle_duration_readout(cx: &mut App) {
    settings::update(cx, |s| {
        let readout = &mut s.prefs.interface.duration_readout;
        *readout = readout.toggled();
    });
}

/// A readout's box: fixed width and tabular digits, so the bar doesn't move as time passes.
fn frame(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .w(px(TIME_WIDTH))
        .h(px(PILL_HEIGHT))
        .flex()
        .items_center()
        .rounded(px(PILL_RADIUS))
        .text_size(px(TEXT_SIZE))
        .font_features(clock::tabular_digits())
        .whitespace_nowrap()
}

/// A readout showing `text` in muted digits.
fn pill(id: &'static str, p: &Palette, text: String) -> Stateful<Div> {
    frame(id).justify_center().text_color(p.text_muted).role(accesskit::Role::Button).child(text)
}

/// Gives a readout its hover pill and a pointer.
fn clickable(pill: Stateful<Div>, p: &Palette) -> Stateful<Div> {
    let hover = p.sunken;
    pill.cursor(CursorStyle::PointingHand).hover(move |style| style.bg(hover))
}
