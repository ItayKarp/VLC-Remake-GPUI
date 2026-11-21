//! The text input's frame: the field, its focus ring and the validation message.

use gpui::{
    Context, CursorStyle, Div, IntoElement, MouseButton, SharedString, Stateful, Window, accesskit,
    div, prelude::*, px,
};

use super::element::TextElement;
use super::keys::CONTEXT;
use super::{TextInput, Validation};
use crate::theme::{self, Palette};
use crate::ui::{focus, metrics};

/// A disabled input is drawn faded, like the kit's other disabled controls.
const DISABLED_OPACITY: f32 = 0.4;

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(window, cx);
        let error = match &self.validation {
            Validation::Error(message) => Some(message.clone()),
            Validation::None => None,
        };
        let field = self.field(&palette, error.is_some(), cx);
        let field = if error.is_some() { field } else { focus::ring(field, &palette) };
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .w_full()
            .child(field)
            .when_some(error, |this, message| this.child(error_message(message, &palette)))
    }
}

impl TextInput {
    /// The framed field, red-bordered when `invalid`.
    fn field(&self, palette: &Palette, invalid: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let enabled = !self.disabled;
        div()
            .id("text-input")
            .key_context(focus::context(&[CONTEXT, focus::CONTROL, focus::TYPING]))
            .track_focus(&self.focus_handle)
            .role(accesskit::Role::TextInput)
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.state.text().to_owned()))
            .aria_placeholder(self.placeholder.clone())
            .h(px(32.))
            .w_full()
            .flex()
            .items_center()
            .px(px(10.))
            .rounded(px(metrics::RADIUS))
            .bg(palette.well)
            .border_2()
            .border_color(if invalid { palette.danger } else { palette.well })
            .text_size(px(14.))
            .line_height(px(20.))
            .text_color(palette.text)
            .when(enabled, |field| Self::interactive(field, cx))
            .when(!enabled, |field| field.opacity(DISABLED_OPACITY))
            .child(TextElement { input: cx.entity() })
    }

    /// Wires the field's actions and mouse handlers; only an enabled input gets them.
    fn interactive(field: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        field
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::backspace_word))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_to_home))
            .on_action(cx.listener(Self::select_to_end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::show_character_palette))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
    }
}

/// The validation message under the field.
fn error_message(message: SharedString, palette: &Palette) -> impl IntoElement {
    div().text_size(px(12.)).line_height(px(16.)).text_color(palette.danger).child(message)
}
