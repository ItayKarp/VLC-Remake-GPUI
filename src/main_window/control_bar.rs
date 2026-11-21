//! The player's control bar (S2.2, `controls-default.png`): 54 px under the video, drawn from a
//! declarative layout (`layout.rs`) and fitted to the window (`fit.rs`), with what doesn't fit
//! in an overflow `…` menu.
//!
//! Every button shows hover, pressed, keyboard focus and disabled states and a tooltip naming
//! its command and shortcut (S2.2.4). Previous and Next are disabled where they would do
//! nothing (the light theme mock's faded transport means that).

pub(super) mod clock;
mod fit;
mod items;
mod layout;
mod play_button;
mod readouts;
mod time_entry;

use gpui::{
    AnyElement, ClickEvent, Context, DismissEvent, Div, Entity, Subscription, Window, div,
    prelude::*, px,
};

use self::fit::{ITEM_GAP, PADDING};
pub(super) use self::layout::{BUTTON, ICON, LEVEL_GAP, SEEK_MIN};
use self::layout::{Item, Slot};
use super::MainWindow;
use crate::assets::icons;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};

/// The control bar's height in the designs.
pub const CONTROL_BAR_HEIGHT: f32 = 54.;
/// The icon size of the overflow button.
const OVERFLOW_ICON: f32 = 20.;

/// The control bar's own state: its layout and the overflow menu.
pub struct ControlBar {
    /// What the bar shows, in order; Customize Player (S8.7) edits it.
    layout: Vec<Slot>,
    /// The overflow menu, while it's open.
    overflow: Option<(Entity<ContextMenu>, Subscription)>,
    overflow_button: BoundsTracker,
    /// The current time while it's being edited (S2.4.1).
    time_edit: Option<readouts::TimeEdit>,
}

impl Default for ControlBar {
    fn default() -> Self {
        Self {
            layout: layout::default_layout(),
            overflow: None,
            overflow_button: BoundsTracker::new(),
            time_edit: None,
        }
    }
}

impl ControlBar {
    /// Closes the overflow menu: another popover opens.
    pub(super) fn close_overflow(&mut self) {
        self.overflow = None;
    }

    /// Whether the overflow menu is open.
    pub(super) fn is_overflow_open(&self) -> bool {
        self.overflow.is_some()
    }
}

impl MainWindow {
    /// The control bar along the bottom of the window.
    pub(super) fn render_control_bar(
        &self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let fitted = fit::fit(&self.control_bar.layout, f32::from(window.viewport_size().width));
        let mut bar = div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(CONTROL_BAR_HEIGHT))
            .px(px(PADDING))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(ITEM_GAP))
            .bg(p.control_bar)
            .occlude();
        for slot in &fitted.slots {
            bar = bar.child(match slot {
                Slot::Item(item) => self.render_item(*item, p, cx),
                Slot::Gap(width) => div().w(px(*width)).flex_none().into_any_element(),
            });
        }
        if fitted.overflow.is_empty() {
            return bar;
        }
        bar.child(self.render_overflow_button(fitted.overflow, cx))
    }

    /// The overflow menu, above its button.
    pub(super) fn render_overflow_menu(&self) -> Option<AnyElement> {
        let (menu, _) = self.control_bar.overflow.as_ref()?;
        let button = self.control_bar.overflow_button.bounds()?;
        Some(popover::attached(button, Side::AboveEnd, 4., 1, menu.clone()))
    }

    fn render_overflow_button(&self, overflow: Vec<Item>, cx: &mut Context<Self>) -> Div {
        let button = IconButton::new("more-controls", icons::MORE, "More controls")
            .size(BUTTON, OVERFLOW_ICON)
            .selected(self.control_bar.overflow.is_some())
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                this.toggle_overflow(&overflow, window, cx);
            }));
        div().relative().flex_none().child(self.control_bar.overflow_button.element()).child(button)
    }

    fn toggle_overflow(&mut self, overflow: &[Item], window: &mut Window, cx: &mut Context<Self>) {
        if self.control_bar.overflow.take().is_some() {
            cx.notify();
            return;
        }
        self.close_popovers(window, cx);
        let entries: Vec<MenuItem> =
            overflow.iter().filter_map(|item| self.overflow_entry(*item, cx)).collect();
        let menu = ContextMenu::build(entries, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.control_bar.overflow = None;
            cx.notify();
        });
        self.control_bar.overflow = Some((menu, subscription));
        cx.notify();
    }
}
