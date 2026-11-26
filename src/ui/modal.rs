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

mod dialog;

use gpui::{
    AnyView, Context, DismissEvent, Div, Entity, EntityId, EventEmitter, FocusHandle, Focusable,
    KeyDownEvent, Stateful, Subscription, WeakFocusHandle, Window, div, prelude::*,
};

use super::focus::{FocusNext, FocusPrev};
use crate::i18n::DirectionalStyle;
use crate::theme::{self, Palette};

pub use dialog::{Dialog, DialogButton};

/// Key context of the top dialog. App and player shortcuts don't fire inside it;
/// a dialog binds its own keys in this context.
pub const CONTEXT: &str = "Modal";

/// What a modal view must provide to go in a [`ModalLayer`].
pub trait ModalView: Render + Focusable + EventEmitter<DismissEvent> {}
impl<T: Render + Focusable + EventEmitter<DismissEvent>> ModalView for T {}

/// One open modal and the focus bookkeeping around it.
struct Entry {
    view: AnyView,
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

/// The stack of open modals in one window, drawn above everything else.
#[derive(Default)]
pub struct ModalLayer {
    stack: Vec<Entry>,
}

impl ModalLayer {
    /// A layer with nothing open.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether any modal is open.
    #[allow(dead_code, reason = "kit API for later stages; used by the tests")]
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

    /// Closes the modal showing entity `id`, giving focus back if it was on top.
    fn close(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
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

    /// Tab: moves focus on, wrapping from the dialog's end back to its start.
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

    /// Shift+Tab: moves focus back, wrapping from the dialog's start to its end.
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

    /// Escape closes the top modal.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
            self.pop(window, cx);
            cx.stop_propagation();
        }
    }

    /// Moves focus to a newly opened modal's first control, once its tab stops exist.
    fn focus_new_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.stack.last_mut() else {
            return;
        };
        if !std::mem::take(&mut entry.focus_pending) {
            return;
        }
        let start = entry.start.clone();
        // Runs once this frame is drawn, when the dialog's tab stops exist.
        window.defer(cx, move |window, cx| {
            window.focus(&start, cx);
            window.focus_next(cx);
        });
    }

    /// The scrim and centered modal for the entry at `index`. Only the top one handles
    /// keys and the focus trap.
    fn layer(&self, index: usize, p: &Palette, cx: &mut Context<Self>) -> Stateful<Div> {
        let entry = &self.stack[index];
        let is_top = index + 1 == self.stack.len();
        div()
            .id(("modal", index))
            .absolute()
            .inset_0()
            .flex_row_start()
            .items_center()
            .justify_center()
            .bg(p.scrim)
            .occlude()
            .when(is_top, |layer| {
                layer
                    .key_context(CONTEXT)
                    .on_action(cx.listener(Self::trap_next))
                    .on_action(cx.listener(Self::trap_prev))
                    .on_key_down(cx.listener(Self::on_key_down))
            })
            .child(
                div()
                    .flex_row_start()
                    .flex_col()
                    .max_w_full()
                    .max_h_full()
                    // The trap's bracketing tab stops.
                    .child(div().track_focus(&entry.start).size_0())
                    .child(entry.view.clone())
                    .child(div().track_focus(&entry.end).size_0()),
            )
    }
}

impl Render for ModalLayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.stack.is_empty() {
            return div().into_any_element();
        }
        let p = theme::palette(window, cx);
        self.focus_new_modal(window, cx);
        let layers = (0..self.stack.len()).map(|i| self.layer(i, &p, cx));
        div().absolute().inset_0().children(layers).into_any_element()
    }
}
