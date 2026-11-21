//! Modal dialogs (S0.4.8).
//!
//! Each window that shows dialogs owns a [`ModalLayer`] and renders it last. The
//! layer holds a stack: nested dialogs are allowed, only the top one is
//! interactive, and Escape closes only the top one. While a dialog is open:
//!
//! - a scrim dims the window and swallows clicks,
//! - Tab and Shift+Tab cycle inside the dialog (focus trap),
//! - Enter triggers the default button unless focus is on another button,
//! - the dialog is capped at the window's size and its body scrolls,
//! - closing returns focus to wherever it was before the dialog opened.
//!
//! The trap works like the web's: two invisible tab stops bracket the dialog.
//! Tabbing onto one of them wraps to the dialog's other end.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyDownEvent, SharedString, Subscription, WeakFocusHandle, Window, accesskit, div, prelude::*,
    px,
};

use super::button::{Button, ButtonStyle};
use super::focus::{FocusNext, FocusPrev};
use super::metrics;
use crate::theme;

type Respond = Rc<dyn Fn(&mut Window, &mut App)>;
type BuildBody = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Space kept between a dialog and the window edges.
const WINDOW_MARGIN: f32 = 24.;

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

    pub fn on_click(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

/// A standard dialog: a title, a body and a row of buttons.
pub struct Dialog {
    title: SharedString,
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
    pub fn new(
        title: impl Into<SharedString>,
        body: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        buttons: Vec<DialogButton>,
        cx: &mut Context<Self>,
    ) -> Self {
        let button_focus = buttons.iter().map(|_| cx.focus_handle().tab_stop(true)).collect();
        Self {
            title: title.into(),
            body: Box::new(body),
            buttons,
            button_focus,
            focus_handle: cx.focus_handle(),
            width: 440.,
        }
    }

    /// The buttons' focus handles, in order.
    #[cfg(test)]
    pub fn button_focus_handles(&self) -> Vec<FocusHandle> {
        self.button_focus.clone()
    }

    fn press(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let on_click = self.buttons[index].on_click.clone();
        cx.emit(DismissEvent);
        if let Some(on_click) = on_click {
            window.defer(cx, move |window, cx| on_click(window, cx));
        }
    }

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
}

impl Render for Dialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let viewport = window.viewport_size();
        let body = (self.body)(window, cx);

        let buttons = self.buttons.iter().enumerate().map(|(i, button)| {
            Button::new(("dialog-button", i), button.label.clone())
                .style(button.style)
                .track_focus(&self.button_focus[i])
                .on_click(cx.listener(move |this, _, window, cx| this.press(i, window, cx)))
        });

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
            .child(
                div()
                    .flex_none()
                    .px(px(24.))
                    .pt(px(20.))
                    .pb(px(12.))
                    .text_size(px(18.))
                    .text_color(p.text_strong)
                    .child(self.title.clone()),
            )
            .child(
                div()
                    .id("dialog-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .text_size(px(metrics::TEXT - 1.))
                    .line_height(px(22.))
                    .child(body),
            )
            .child(
                div()
                    .flex_none()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap(px(8.))
                    .px(px(24.))
                    .pt(px(16.))
                    .pb(px(20.))
                    .children(buttons),
            )
    }
}

/// What a modal view must provide to go in a [`ModalLayer`].
pub trait ModalView: Render + Focusable + EventEmitter<DismissEvent> {}
impl<T: Render + Focusable + EventEmitter<DismissEvent>> ModalView for T {}

struct Entry {
    view: gpui::AnyView,
    focus: FocusHandle,
    /// Where focus was before this modal opened.
    restore: Option<WeakFocusHandle>,
    start: FocusHandle,
    end: FocusHandle,
    /// Focus moves to the first control after the frame that first draws the
    /// modal, when its tab stops exist.
    focus_pending: bool,
    _subscription: Subscription,
}

#[derive(Default)]
pub struct ModalLayer {
    stack: Vec<Entry>,
}

impl ModalLayer {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(dead_code)] // Kit API for later stages; used by the tests.
    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Opens `view` above any open modals and moves focus into it.
    pub fn push<V: ModalView>(
        &mut self,
        view: Entity<V>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let restore = window.focused(cx).map(|f| f.downgrade());
        let focus = view.focus_handle(cx);
        let subscription = cx.subscribe_in(&view, window, {
            let id = view.entity_id();
            move |this, _, _: &DismissEvent, window, cx| this.close(id, window, cx)
        });
        self.stack.push(Entry {
            view: view.into(),
            focus: focus.clone(),
            restore,
            start: cx.focus_handle().tab_stop(true),
            end: cx.focus_handle().tab_stop(true),
            focus_pending: true,
            _subscription: subscription,
        });
        focus.focus(window, cx);
        cx.notify();
    }

    /// Closes the top modal, as Escape does.
    pub fn pop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.stack.last().map(|e| e.view.entity_id()) {
            self.close(id, window, cx);
        }
    }

    fn close(&mut self, id: gpui::EntityId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = self.stack.iter().position(|e| e.view.entity_id() == id) else {
            return;
        };
        let entry = self.stack.remove(index);
        if index == self.stack.len() {
            // It was the top one: give focus back.
            if let Some(handle) = entry.restore.and_then(|h| h.upgrade()) {
                window.focus(&handle, cx);
            } else if let Some(top) = self.stack.last() {
                window.focus(&top.focus, cx);
            }
        }
        cx.notify();
    }

    fn trap_next(&mut self, _: &FocusNext, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_next(cx);
        if let Some(top) = self.stack.last()
            && (top.end.is_focused(window) || !top.focus.contains_focused(window, cx))
            && !top.start.is_focused(window)
        {
            window.focus(&top.start, cx);
            window.focus_next(cx);
        }
    }

    fn trap_prev(&mut self, _: &FocusPrev, window: &mut Window, cx: &mut Context<Self>) {
        window.focus_prev(cx);
        if let Some(top) = self.stack.last()
            && (top.start.is_focused(window) || !top.focus.contains_focused(window, cx))
            && !top.end.is_focused(window)
        {
            window.focus(&top.end, cx);
            window.focus_prev(cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
            self.pop(window, cx);
            cx.stop_propagation();
        }
    }
}

impl Render for ModalLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.stack.is_empty() {
            return div().into_any_element();
        }
        let p = theme::palette(window, cx);
        let top = self.stack.len() - 1;
        if let Some(entry) = self.stack.last_mut()
            && std::mem::take(&mut entry.focus_pending)
        {
            let start = entry.start.clone();
            // Runs once this frame is drawn, when the dialog's tab stops exist.
            window.defer(cx, move |window, cx| {
                window.focus(&start, cx);
                window.focus_next(cx);
            });
        }

        let layers = self.stack.iter().enumerate().map(|(i, entry)| {
            let is_top = i == top;
            div()
                .id(("modal", i))
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(p.scrim)
                .occlude()
                .when(is_top, |layer| {
                    layer
                        .key_context("Modal")
                        .on_action(cx.listener(Self::trap_next))
                        .on_action(cx.listener(Self::trap_prev))
                        .on_key_down(cx.listener(Self::on_key_down))
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .max_w_full()
                        .max_h_full()
                        // The trap's bracketing tab stops.
                        .child(div().track_focus(&entry.start).size_0())
                        .child(entry.view.clone())
                        .child(div().track_focus(&entry.end).size_0()),
                )
        });

        div().absolute().inset_0().children(layers).into_any_element()
    }
}
