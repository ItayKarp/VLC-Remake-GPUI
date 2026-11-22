//! A window showing every kit component, for design checks and screenshots
//! (S0.4 Definition of Done). Open it with `--component-gallery` (`app::cli::COMPONENT_GALLERY`).
//!
//! The theme and accent switches at the top change the real settings, so the
//! gallery shows exactly what the app will look like; point `RVLC_CONFIG_DIR` at
//! a scratch folder to keep your own settings untouched.

mod basics;
mod controls;
mod dialog;
mod events;
mod menu;
mod overlays;
mod settings_page;
mod widgets;

use gpui::{
    App, Context, Entity, FocusHandle, KeyDownEvent, Pixels, Point, SharedString, Subscription,
    Window, div, prelude::*, px,
};

use self::settings_page::SettingsDemo;
use self::widgets::Widgets;
use super::menu::ContextMenu;
use super::popover::{self, BoundsTracker};
use super::toast::Toast;
use super::{focus, metrics};
use crate::theme::{self, Palette};

/// The component kit gallery window: one of every kit component, wired to demo state.
pub struct Gallery {
    focus_handle: FocusHandle,
    values: ControlValues,
    section_open: bool,
    banner_visible: bool,
    widgets: Widgets,
    settings_demo: SettingsDemo,
    menu: Option<(Point<Pixels>, Entity<ContextMenu>, Subscription)>,
    popover: PopoverState,
    status: SharedString,
    _subscriptions: Vec<Subscription>,
}

/// The two demo toggles, one starting on and one starting off.
struct Toggles {
    on: bool,
    off: bool,
}

/// What the gallery's stateless demo controls currently show.
struct ControlValues {
    toggles: Toggles,
    remember_position: bool,
    unit: usize,
    value: usize,
    starting_volume: usize,
    tab: usize,
}

impl Default for ControlValues {
    fn default() -> Self {
        Self {
            toggles: Toggles { on: true, off: false },
            remember_position: true,
            unit: 0,
            value: 1,
            starting_volume: 1,
            tab: 2,
        }
    }
}

/// The demo popover: whether it is open, and where its trigger button is.
struct PopoverState {
    open: bool,
    trigger: BoundsTracker,
}

impl Gallery {
    /// Builds the gallery's components and focuses the window so Tab works at once.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::observe(window, cx);
        let widgets = Widgets::new(window, cx);
        let subscriptions = events::subscribe(&widgets, cx);
        let settings_demo = SettingsDemo::new(window, cx);
        popover::close_on_deactivate(window, cx, |this: &mut Self, _, cx| {
            this.popover.open = false;
            cx.notify();
        });

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            focus_handle,
            values: ControlValues::default(),
            section_open: true,
            banner_visible: true,
            widgets,
            settings_demo,
            menu: None,
            popover: PopoverState { open: false, trigger: BoundsTracker::new() },
            status: "Tab through the controls; arrow keys move inside groups.".into(),
            _subscriptions: subscriptions,
        }
    }

    /// Shows `status` in the header, which is how the demo reports what a control did.
    fn set_status(&mut self, status: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.status = status.into();
        cx.notify();
    }

    /// Shows a toast in the gallery's toast stack.
    fn push_toast(&self, toast: Toast, cx: &mut App) {
        self.widgets.toasts.update(cx, |toasts, cx| toasts.push(toast, cx));
    }

    /// Escape closes the popover, the way it does in the player's control bar.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && self.popover.open {
            self.popover.open = false;
            cx.notify();
        }
    }

    /// The page title on the left and the latest status message on the right.
    fn header(&self, p: &Palette) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .child(div().text_size(px(22.)).text_color(p.text_strong).child("Component kit"))
            .child(div().text_color(p.text_muted).child(self.status.clone()))
    }

    /// The scrolling column of component sections.
    fn sections(&self, p: &Palette, cx: &Context<Self>) -> impl IntoElement {
        div()
            .id("gallery-scroll")
            .size_full()
            .overflow_y_scroll()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(self.header(p))
            .child(basics::appearance(p, cx))
            .child(basics::buttons(p))
            .child(self.toggles(p, cx))
            .child(self.pickers(p, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(16.))
                    .items_start()
                    .child(div().flex_1().child(self.sliders(p)))
                    .child(self.panel(p, cx)),
            )
            .child(self.overlays(p, cx))
            .child(self.grid(p))
            .child(self.settings_demo.render(p))
    }
}

/// Wraps a state change as an element listener that also re-renders the gallery.
fn updating<E: ?Sized>(
    cx: &Context<Gallery>,
    change: impl Fn(&mut Gallery, &E) + 'static,
) -> impl Fn(&E, &mut Window, &mut App) + 'static {
    cx.listener(move |this, event, _, cx| {
        change(this, event);
        cx.notify();
    })
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let popover = self.popover_element(&p, window, cx);
        let menu = self
            .menu
            .as_ref()
            .map(|(position, menu, _)| popover::at_position(*position, 1, menu.clone()));

        let root = div()
            .id("gallery")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .bg(p.background)
            .text_color(p.text)
            .text_size(px(metrics::TEXT - 2.))
            .on_key_down(cx.listener(Self::on_key_down))
            .child(self.sections(&p, cx))
            .children(popover)
            .children(menu)
            .child(self.widgets.toasts.clone())
            .child(self.widgets.modals.clone());
        focus::tab_navigation(root)
    }
}
