//! Shared helpers for the kit's window tests: app setup, painting a frame, and a
//! host window around the view under test.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AppContext, Context, Entity, FocusHandle, Render, TestAppContext, VisualTestContext, Window,
    div, prelude::*,
};

/// Installs the globals the kit needs: settings, theme and key bindings.
pub(super) fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
    });
}

/// Paints a frame (so focus, tab stops and bounds exist) and settles tasks.
pub(super) fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

/// A window that shows one child view, with Tab handling like the app's windows.
struct Host {
    child: gpui::AnyView,
    focus: FocusHandle,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::ui::focus::tab_navigation(
            div().track_focus(&self.focus).size_full().child(self.child.clone()),
        )
    }
}

/// Opens a window showing the view `build` makes, paints it, and returns the view.
pub(super) fn host<V: Render>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
) -> (Entity<V>, &mut VisualTestContext) {
    let slot: Rc<RefCell<Option<Entity<V>>>> = Rc::default();
    let out = slot.clone();
    let (_, cx) = cx.add_window_view(move |window, cx| {
        let child = cx.new(|cx| build(window, cx));
        *out.borrow_mut() = Some(child.clone());
        Host { child: child.into(), focus: cx.focus_handle() }
    });
    let child = slot.borrow_mut().take().unwrap();
    draw(cx);
    (child, cx)
}
