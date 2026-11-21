//! How the gallery reacts to its components' events: it reports them in the status line,
//! and validates the playlist name as it is typed.

use gpui::{Context, Entity, Subscription};

use super::Gallery;
use super::widgets::Widgets;
use crate::ui::text_input::{self, TextInput, Validation};
use crate::ui::{dropdown, list, slider};

/// Subscribes the gallery to the events of the components it reports on.
pub(super) fn subscribe(widgets: &Widgets, cx: &mut Context<Gallery>) -> Vec<Subscription> {
    vec![
        cx.subscribe(&widgets.dropdown, |this, _, event: &dropdown::Changed, cx| {
            this.set_status(format!("Dropdown → item {}", event.0), cx);
        }),
        cx.subscribe(&widgets.sliders.delay, |this, _, event: &slider::Changed, cx| {
            this.set_status(format!("Delay → {:.3} s", event.0), cx);
        }),
        cx.subscribe(&widgets.grid, |this, _, event: &list::Event, cx| {
            this.set_status(format!("Grid → {event:?}"), cx);
        }),
        cx.subscribe(&widgets.name, |this, input, event, cx| this.on_name_event(&input, event, cx)),
    ]
}

impl Gallery {
    fn on_name_event(
        &mut self,
        input: &Entity<TextInput>,
        event: &text_input::Event,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, text_input::Event::Changed) {
            return;
        }
        let text = input.read(cx).text().to_owned();
        let validation = name_validation(&text);
        input.update(cx, |input, cx| input.set_validation(validation, cx));
        self.set_status(format!("Name → {text:?}"), cx);
    }
}

/// Playlist names become file names, so they can't hold path separators.
fn name_validation(text: &str) -> Validation {
    if text.chars().any(|c| matches!(c, '/' | '\\' | ':')) {
        Validation::Error("Names can't contain / \\ or :".into())
    } else {
        Validation::None
    }
}
