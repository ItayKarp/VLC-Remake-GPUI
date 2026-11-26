//! Context menus (S0.4.7): `library/menus/*` and the title bar's `…` menu.
//!
//! Items: actions with an optional icon and shortcut hint, check items, radio
//! groups, submenus, separators and disabled items, which say why on hover. Up/Down/Home/End move the
//! highlight (skipping separators and disabled items, wrapping), Enter/Space
//! activate, Right opens a submenu and Left/Escape close it, and typing a letter
//! jumps to the next item starting with it. The menu closes on activation,
//! outside click, Escape and window deactivation, by emitting [`DismissEvent`].
//! Closing gives the keyboard back to whatever had it when the menu opened, so a
//! window's shortcuts keep working after a pick at any depth.
//!
//! **Native popups.** `WindowKind::AnchoredPopup` could let a menu extend past
//! the window edge, but it's a separate OS window: it takes focus from the main
//! window (deactivating it, which closes menus), needs its own theme observers,
//! and isn't positioned on Wayland. In-window menus that flip and scroll cover
//! the designs, so that's what the kit uses; revisit if a design needs a menu
//! larger than the window.
//!
//! Usage: create with [`ContextMenu::build`], subscribe to `DismissEvent` to drop
//! it, and render it with `popover::at_position` or `popover::attached`.

mod item;
mod keys;
mod render;

use gpui::{
    App, AppContext, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, Pixels,
    Point, ScrollHandle, Subscription, Window,
};

pub use item::MenuItem;

use super::dropdown::TypeAhead;
use super::focus::{self, Layout, Move};
use super::popover::{self, BoundsTracker};
use crate::theme;

/// Narrowest a menu gets, in logical pixels.
const MIN_WIDTH: f32 = 220.;

/// A submenu that is showing, and which of this menu's items opened it.
struct OpenSubmenu {
    index: usize,
    menu: Entity<ContextMenu>,
    _subscription: Subscription,
}

/// A popup menu of [`MenuItem`]s; emits [`DismissEvent`] when it should close.
pub struct ContextMenu {
    items: Vec<MenuItem>,
    highlighted: Option<usize>,
    focus_handle: FocusHandle,
    submenu: Option<OpenSubmenu>,
    /// Bounds of each row, to attach submenus to.
    rows: Vec<BoundsTracker>,
    /// This menu's own bounds, so a parent can tell clicks inside its submenu.
    tracker: BoundsTracker,
    scroll: ScrollHandle,
    type_ahead: TypeAhead,
    /// Nesting depth: draws submenus above their parents.
    depth: usize,
    min_width: f32,
    /// What had focus when the root menu opened, to give it back on close.
    /// Submenus leave this to the root.
    restore_focus: Option<FocusHandle>,
}

impl EventEmitter<DismissEvent> for ContextMenu {}

impl Focusable for ContextMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ContextMenu {
    /// Creates a menu that takes keyboard focus as soon as it opens.
    pub fn build(items: Vec<MenuItem>, window: &mut Window, cx: &mut App) -> Entity<ContextMenu> {
        Self::build_at_depth(items, 0, window, cx)
    }

    fn build_at_depth(
        items: Vec<MenuItem>,
        depth: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<ContextMenu> {
        let restore_focus = if depth == 0 { window.focused(cx) } else { None };
        cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            // Take focus once the event that opened the menu has finished: a click
            // focuses the clicked element's view after its handlers run.
            let handle = focus_handle.clone();
            window.defer(cx, move |window, cx| handle.focus(window, cx));
            theme::observe(window, cx);
            popover::close_on_deactivate(window, cx, Self::dismiss);
            let rows = items.iter().map(|_| BoundsTracker::new()).collect();
            Self {
                items,
                highlighted: None,
                focus_handle,
                submenu: None,
                rows,
                tracker: BoundsTracker::new(),
                scroll: ScrollHandle::new(),
                type_ahead: TypeAhead::default(),
                depth,
                min_width: MIN_WIDTH,
                restore_focus,
            }
        })
    }

    /// Whether `position` is over this menu or any of its open submenus.
    pub fn contains(&self, position: Point<Pixels>, cx: &App) -> bool {
        self.tracker.contains(position)
            || self.submenu.as_ref().is_some_and(|s| s.menu.read(cx).contains(position, cx))
    }

    /// Starts with the first item highlighted, for menus opened from the keyboard.
    pub fn highlight_first(&mut self, cx: &mut Context<Self>) {
        self.move_highlight(Move::First, cx);
    }

    /// Each item's label and whether it can be chosen, in order; separators are left out.
    #[cfg(test)]
    pub fn entries(&self) -> Vec<(String, bool)> {
        let entries = self.items.iter().filter_map(|item| Some((item.label()?, item.selectable())));
        entries.map(|(label, selectable)| (label.to_string(), selectable)).collect()
    }

    fn move_highlight(&mut self, movement: Move, cx: &mut Context<Self>) {
        let items = &self.items;
        self.highlighted =
            focus::step(self.highlighted, items.len(), movement, Layout::WRAPPING, |i| {
                items[i].selectable()
            });
        if let Some(index) = self.highlighted {
            self.scroll.scroll_to_item(index);
        }
        cx.notify();
    }

    /// Runs the item at `index` (after the menu closes) or opens its submenu.
    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        match &self.items[index] {
            MenuItem::Action { on_select, disabled: false, .. }
            | MenuItem::Check { on_select, disabled: false, .. } => {
                let on_select = on_select.clone();
                self.dismiss(window, cx);
                // Run after the menu is gone, so the action can open dialogs or
                // move focus without the menu in the way.
                window.defer(cx, move |window, cx| on_select(window, cx));
            }
            MenuItem::Submenu { .. } => self.open_submenu(index, true, window, cx),
            _ => {}
        }
    }

    /// Opens the submenu of the item at `index`. From the keyboard, the submenu
    /// takes focus with its first item highlighted; from hover, focus stays here.
    fn open_submenu(
        &mut self,
        index: usize,
        from_keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.submenu.as_ref().is_some_and(|s| s.index == index) {
            if from_keyboard {
                self.focus_submenu(window, cx);
            }
            return;
        }
        let MenuItem::Submenu { items, disabled: false, .. } = &mut self.items[index] else {
            return;
        };
        // Submenus are rebuilt each time they open, so take the items and put
        // them back when it closes.
        let items = std::mem::take(items);
        let menu = Self::build_at_depth(items, self.depth + 1, window, cx);
        if from_keyboard {
            menu.update(cx, ContextMenu::highlight_first);
        } else {
            // Hovering shouldn't steal the keyboard from this menu.
            self.focus_handle.focus(window, cx);
        }
        let subscription =
            cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
                // An item in the submenu was chosen: close the whole chain.
                this.submenu = None;
                this.dismiss(window, cx);
            });
        self.close_submenu(window, cx);
        self.submenu = Some(OpenSubmenu { index, menu, _subscription: subscription });
        cx.notify();
    }

    /// Closes the menu: gives the keyboard back to what had it when the menu opened
    /// (unless it has already moved outside the menu), then emits [`DismissEvent`].
    ///
    /// Without this, focus stays on the menu's handle, which is dropped with the menu,
    /// and the window's keys stop reaching the view that had them.
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus_in_menu = self.focus_handle.contains_focused(window, cx);
        if let Some(previous) = self.restore_focus.take()
            && (focus_in_menu || window.focused(cx).is_none())
        {
            previous.focus(window, cx);
        }
        cx.emit(DismissEvent);
    }

    /// Moves the keyboard into the open submenu, highlighting its first item.
    fn focus_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(open) = &self.submenu else {
            return;
        };
        open.menu.update(cx, |menu, cx| {
            menu.focus_handle.focus(window, cx);
            menu.highlight_first(cx);
        });
    }

    /// Closes the open submenu (if any), returning its items to this menu.
    fn close_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.submenu.take() {
            let items = open.menu.update(cx, |menu, _| std::mem::take(&mut menu.items));
            if let MenuItem::Submenu { items: slot, .. } = &mut self.items[open.index] {
                *slot = items;
            }
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }
}
