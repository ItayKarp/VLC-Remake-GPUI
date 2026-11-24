//! The text input's frame: the field, its focus ring and the validation message.

use gpui::{
    Context, CursorStyle, Div, IntoElement, MouseButton, SharedString, Stateful, Window, accesskit,
    div, prelude::*, px,
};

use super::element::TextElement;
use super::keys::{CONTEXT, MULTI_LINE_CONTEXT};
use super::{Look, TextInput, Validation};
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::{focus, metrics};

/// A disabled input is drawn faded, like the kit's other disabled controls.
const DISABLED_OPACITY: f32 = 0.4;

/// The framed field's text size and line height.
const TEXT_SIZE: f32 = 14.;
const LINE_HEIGHT: f32 = 20.;
/// The framed field's height with one line, and the space around a multi-line field's text.
const SINGLE_LINE_HEIGHT: f32 = 32.;
const MULTI_LINE_PADDING: f32 = 10.;

impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = theme::palette(window, cx);
        if self.look == Look::Bare {
            return self.bare(&palette, cx);
        }
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
    /// The text alone, red when invalid ([`Look::Bare`]).
    fn bare(&self, palette: &Palette, cx: &mut Context<Self>) -> Div {
        let invalid = matches!(self.validation, Validation::Error(_));
        let field = self
            .base()
            .w_full()
            .when(invalid, |field| field.text_color(palette.danger))
            .when(!self.disabled, |field| self.interactive(field, cx))
            .child(TextElement { input: cx.entity() });
        div().w_full().child(field)
    }

    /// The focusable, labelled element both looks build on.
    fn base(&self) -> Stateful<Div> {
        let context = if self.rows.is_some() {
            focus::context(&[CONTEXT, MULTI_LINE_CONTEXT, focus::CONTROL, focus::TYPING])
        } else {
            focus::context(&[CONTEXT, focus::CONTROL, focus::TYPING])
        };
        div()
            .id("text-input")
            .key_context(context)
            .track_focus(&self.focus_handle)
            .role(accesskit::Role::TextInput)
            .aria_label(self.label.clone())
            .aria_value(SharedString::from(self.state.text().to_owned()))
            .aria_placeholder(self.placeholder.clone())
    }

    /// The framed field, red-bordered when `invalid`.
    fn field(&self, palette: &Palette, invalid: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        let enabled = !self.disabled;
        let field = match self.rows {
            Some(rows) => self
                .base()
                .h(px(rows as f32 * LINE_HEIGHT + 2. * MULTI_LINE_PADDING))
                .py(px(MULTI_LINE_PADDING - 2.))
                .items_start(),
            None => self.base().h(px(SINGLE_LINE_HEIGHT)).items_center(),
        };
        field
            .w_full()
            .flex()
            .px(px(10.))
            .rounded(px(metrics::RADIUS))
            .bg(palette.well)
            .border_2()
            .border_color(if invalid { palette.danger } else { palette.well })
            .text_size(text_px(TEXT_SIZE))
            .line_height(text_px(LINE_HEIGHT))
            .text_color(palette.text)
            .when(enabled, |field| self.interactive(field, cx))
            .when(!enabled, |field| field.opacity(DISABLED_OPACITY))
            .child(TextElement { input: cx.entity() })
    }

    /// Wires the field's actions and mouse handlers; only an enabled input gets them.
    fn interactive(&self, field: Stateful<Div>, cx: &mut Context<Self>) -> Stateful<Div> {
        let field = if self.rows.is_some() { Self::multi_line_actions(field, cx) } else { field };
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
    div()
        .text_size(text_px(12.))
        .line_height(text_px(16.))
        .text_color(palette.danger)
        .child(message)
}
