//! The Playback tab's Sleep Timer row (S8.13.1, `controls-playback.png`): a dropdown of Off,
//! 15 to 90 minutes, End of current item and Custom. Custom shows a Minutes field under it,
//! which starts the timer on Enter; while a timer runs, a line says when it stops.

use gpui::{
    Context, Div, Entity, Focusable as _, SharedString, Subscription, Window, div, prelude::*, px,
};

use crate::i18n::DirectionalStyle;
use crate::main_window::MainWindow;
use crate::main_window::sleep_timer::rules::{self, Choice, MAX_MINUTES};
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::text_input::{self, TextInput, Validation};

/// The row's label, height and text size, the dropdown's and field's width, and the hint's
/// text size.
const LABEL: &str = tr_noop!("Sleep Timer");
const ROW_HEIGHT: f32 = 68.;
const ROW_TEXT: f32 = 14.;
const CONTROL_WIDTH: f32 = 220.;
const HINT_TEXT: f32 = 12.;
/// The Custom field's label and height.
const MINUTES_LABEL: &str = tr_noop!("Minutes");
const FIELD_ROW_HEIGHT: f32 = 52.;

/// The sleep timer's dropdown and Custom field.
pub(in crate::main_window) struct SleepTimerControl {
    dropdown: Entity<Dropdown>,
    minutes: Entity<TextInput>,
    _events: [Subscription; 2],
}

impl SleepTimerControl {
    /// The dropdown on Off, and an empty Custom field.
    pub(in crate::main_window) fn new(window: &mut Window, cx: &mut Context<MainWindow>) -> Self {
        let labels = Choice::ALL.iter().map(|choice| SharedString::from(choice.label()));
        let label = crate::i18n::text(LABEL);
        let dropdown = cx.new(|cx| Dropdown::new(label, labels.collect(), 0, window, cx));
        let minutes = cx.new(|cx| {
            TextInput::new(window, cx).with_placeholder("20").with_label(tr!("Sleep timer minutes"))
        });
        let picked = cx.subscribe_in(&dropdown, window, MainWindow::on_sleep_timer_picked);
        let typed = cx.subscribe_in(&minutes, window, MainWindow::on_sleep_minutes);
        Self { dropdown, minutes, _events: [picked, typed] }
    }
}

impl MainWindow {
    /// The row, the Custom field while Custom is picked, and when the timer stops.
    pub(super) fn render_sleep_timer(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let control = &self.settings_panel.sleep_timer;
        let choice = self.sleep_choice();
        let selected = Choice::ALL.iter().position(|shown| *shown == choice).unwrap_or_default();
        control.dropdown.update(cx, |dropdown, cx| dropdown.set_selected(selected, cx));
        let label = crate::i18n::text(LABEL);
        let timer = row(label, ROW_HEIGHT, p).child(sized(control.dropdown.clone()));
        let minutes = crate::i18n::text(MINUTES_LABEL);
        let custom = (choice == Choice::Custom)
            .then(|| row(minutes, FIELD_ROW_HEIGHT, p).child(sized(control.minutes.clone())));
        let hint = self.sleep_hint(cx).map(|hint| {
            div()
                .ps(px(4.))
                .pb(px(8.))
                .text_size(text_px(HINT_TEXT))
                .text_color(p.text_muted)
                .child(hint)
        });
        div().flex_row_start().flex_col().child(timer).children(custom).children(hint)
    }

    fn on_sleep_timer_picked(
        &mut self,
        _: &Entity<Dropdown>,
        event: &dropdown::Changed,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(&choice) = Choice::ALL.get(event.0) else { return };
        self.pick_sleep_timer(choice, cx);
        if choice == Choice::Custom {
            let handle = self.settings_panel.sleep_timer.minutes.focus_handle(cx);
            handle.focus(window, cx);
        }
    }

    fn on_sleep_minutes(
        &mut self,
        field: &Entity<TextInput>,
        event: &text_input::Event,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            text_input::Event::Changed => {
                field.update(cx, |field, cx| field.set_validation(Validation::None, cx));
            }
            text_input::Event::Submitted => match rules::parse_minutes(field.read(cx).text()) {
                Ok(minutes) => self.start_sleep_minutes(minutes, Choice::Custom, cx),
                Err(rules::Invalid) => field.update(cx, |field, cx| {
                    let why = tr!("1 to {most} minutes", most = MAX_MINUTES);
                    field.set_validation(Validation::Error(why.into()), cx);
                }),
            },
            text_input::Event::Blurred => {}
        }
    }
}

/// A row: its label, then its control.
fn row(label: &'static str, height: f32, p: &Palette) -> Div {
    div()
        .h(px(height))
        .flex_row_start()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .ps(px(4.))
        .text_size(text_px(ROW_TEXT))
        .text_color(p.text_muted)
        .child(label)
}

/// A control at the rows' width.
fn sized(control: impl IntoElement) -> Div {
    div().w(px(CONTROL_WIDTH)).flex_none().child(control)
}

#[cfg(test)]
mod tests;
