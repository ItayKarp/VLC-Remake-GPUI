//! The standard dialog shown in a [`super::ModalLayer`]: a title, an optional toolbar that
//! stays put (a search field), a scrolling body and a row of buttons, one of which Enter can
//! trigger.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, DismissEvent, Div, EventEmitter, FocusHandle, Focusable,
    KeyDownEvent, SharedString, Stateful, Window, accesskit, div, prelude::*, px,
};

use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::metrics;

/// Runs when a dialog button is pressed, after the dialog has closed.
type Respond = Rc<dyn Fn(&mut Window, &mut App)>;
/// Builds the dialog's body each frame.
type BuildBody = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Space kept between a dialog and the window edges.
const WINDOW_MARGIN: f32 = 24.;
/// A dialog's width when the window has room for it.
const DIALOG_WIDTH: f32 = 440.;
/// Horizontal padding inside the dialog.
const PADDING: f32 = 24.;

/// A button in a dialog's button row. Every dialog button closes the dialog.
pub struct DialogButton {
    label: SharedString,
    style: ButtonStyle,
    /// Enter triggers it when focus isn't on another button.
    default: bool,
    on_click: Option<Respond>,
}

impl DialogButton {
    /// A button that closes the dialog after running `on_click`.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self { label: label.into(), style: ButtonStyle::Filled, default: false, on_click: None }
    }

    /// The dialog's default (accent) button.
    pub fn primary(mut self) -> Self {
        self.style = ButtonStyle::Primary;
        self.default = true;
        self
    }

    /// Runs `f` once the dialog has closed.
    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

/// A standard dialog: a title, a body and a row of buttons.
pub struct Dialog {
    title: SharedString,
    toolbar: Option<BuildBody>,
    body: BuildBody,
    buttons: Vec<DialogButton>,
    button_focus: Vec<FocusHandle>,
    focus_handle: FocusHandle,
    width: f32,
}

impl EventEmitter<DismissEvent> for Dialog {}

impl Focusable for Dialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Dialog {
    /// A dialog titled `title` whose body `body` builds, with `buttons` in a row at
    /// the bottom (in order, left to right).
    pub fn new(
        title: impl Into<SharedString>,
        body: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        buttons: Vec<DialogButton>,
        cx: &mut Context<Self>,
    ) -> Self {
        let button_focus = buttons.iter().map(|_| cx.focus_handle().tab_stop(true)).collect();
        Self {
            title: title.into(),
            toolbar: None,
            body: Box::new(body),
            buttons,
            button_focus,
            focus_handle: cx.focus_handle(),
            width: DIALOG_WIDTH,
        }
    }

    /// A dialog `width` wide when the window has room (440 px otherwise).
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Puts what `toolbar` builds between the title and the body, where it doesn't scroll.
    pub fn with_toolbar(
        mut self,
        toolbar: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        self.toolbar = Some(Box::new(toolbar));
        self
    }

    /// The buttons' focus handles, in order.
    #[cfg(test)]
    pub fn button_focus_handles(&self) -> Vec<FocusHandle> {
        self.button_focus.clone()
    }

    /// Closes the dialog, then runs the button's action.
    fn press(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let on_click = self.buttons[index].on_click.clone();
        cx.emit(DismissEvent);
        if let Some(on_click) = on_click {
            window.defer(cx, move |window, cx| on_click(window, cx));
        }
    }

    /// Enter presses the default button, unless a button has focus.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key != "enter" || event.keystroke.modifiers.modified() {
            return;
        }
        // A focused button clicks itself on Enter.
        if self.button_focus.iter().any(|h| h.is_focused(window)) {
            return;
        }
        if let Some(index) = self.buttons.iter().position(|b| b.default) {
            self.press(index, window, cx);
            cx.stop_propagation();
        }
    }

    /// The row of buttons along the bottom.
    fn button_row(&self, cx: &mut Context<Self>) -> Div {
        let buttons = self.buttons.iter().enumerate().map(|(i, button)| {
            Button::new(("dialog-button", i), button.label.clone())
                .style(button.style)
                .track_focus(&self.button_focus[i])
                .on_click(cx.listener(move |this, _, window, cx| this.press(i, window, cx)))
        });
        div()
            .flex_none()
            .flex()
            .flex_row()
            .justify_end()
            .gap(px(8.))
            .px(px(PADDING))
            .pt(px(16.))
            .pb(px(20.))
            .children(buttons)
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let viewport = window.viewport_size();
        let toolbar = self.toolbar.as_ref().map(|toolbar| toolbar(window, cx));
        let body = (self.body)(window, cx);

        div()
            .id("dialog")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .role(accesskit::Role::Dialog)
            .aria_label(self.title.clone())
            .w(px(self.width))
            .max_w(viewport.width - px(2. * WINDOW_MARGIN))
            .max_h(viewport.height - px(2. * WINDOW_MARGIN))
            .flex()
            .flex_col()
            .rounded(px(metrics::RADIUS_LARGE + 4.))
            .bg(p.popover)
            .border_1()
            .border_color(p.divider)
            .shadow_lg()
            .text_color(p.text)
            .child(title(self.title.clone(), &p))
            .children(toolbar.map(|toolbar| div().flex_none().px(px(PADDING)).child(toolbar)))
            .child(scrolling_body(body))
            .child(self.button_row(cx))
    }
}

/// The dialog's title line.
fn title(text: SharedString, p: &Palette) -> Div {
    div()
        .flex_none()
        .px(px(PADDING))
        .pt(px(20.))
        .pb(px(12.))
        .text_size(px(18.))
        .text_color(p.text_strong)
        .child(text)
}

/// The body, which scrolls when the dialog is capped at the window's height.
fn scrolling_body(body: AnyElement) -> Stateful<Div> {
    div()
        .id("dialog-body")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .px(px(PADDING))
        .text_size(px(metrics::TEXT - 1.))
        .line_height(px(22.))
        .child(body)
}
