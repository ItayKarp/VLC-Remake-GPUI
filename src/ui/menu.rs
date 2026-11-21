//! Context menus (S0.4.7): `library/menus/*` and the title bar's `…` menu.
//!
//! Items: actions with an optional icon and shortcut hint, check items, radio
//! groups, submenus, separators and disabled items. Up/Down/Home/End move the
//! highlight (skipping separators and disabled items, wrapping), Enter/Space
//! activate, Right opens a submenu and Left/Escape close it, and typing a letter
//! jumps to the next item starting with it. The menu closes on activation,
//! outside click, Escape and window deactivation, by emitting [`DismissEvent`].
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

use std::rc::Rc;
use std::time::Instant;

use gpui::{
    App, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    ScrollHandle, SharedString, Subscription, Window, div, prelude::*, px,
};

use super::dropdown::TypeAhead;
use super::focus::{self, Axis, Layout, Move};
use super::metrics;
use super::option_row::OptionRow;
use super::popover::{self, BoundsTracker, Side};
use crate::theme;

type Activate = Rc<dyn Fn(&mut Window, &mut App)>;

pub enum MenuItem {
    Action {
        label: SharedString,
        icon: Option<&'static str>,
        shortcut: Option<SharedString>,
        disabled: bool,
        on_select: Activate,
    },
    /// A check item. Radio items are check items too: several in a row with one
    /// checked form a group, and the designs mark both with a ✓.
    Check {
        label: SharedString,
        checked: bool,
        disabled: bool,
        on_select: Activate,
    },
    Submenu {
        label: SharedString,
        icon: Option<&'static str>,
        items: Vec<MenuItem>,
    },
    Separator,
}

impl MenuItem {
    pub fn action(
        label: impl Into<SharedString>,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        MenuItem::Action {
            label: label.into(),
            icon: None,
            shortcut: None,
            disabled: false,
            on_select: Rc::new(on_select),
        }
    }

    pub fn check(
        label: impl Into<SharedString>,
        checked: bool,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        MenuItem::Check {
            label: label.into(),
            checked,
            disabled: false,
            on_select: Rc::new(on_select),
        }
    }

    pub fn radio(
        label: impl Into<SharedString>,
        selected: bool,
        on_select: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        MenuItem::Check {
            label: label.into(),
            checked: selected,
            disabled: false,
            on_select: Rc::new(on_select),
        }
    }

    pub fn submenu(label: impl Into<SharedString>, items: Vec<MenuItem>) -> Self {
        MenuItem::Submenu {
            label: label.into(),
            icon: None,
            items,
        }
    }

    pub fn icon(mut self, path: &'static str) -> Self {
        match &mut self {
            MenuItem::Action { icon, .. } | MenuItem::Submenu { icon, .. } => *icon = Some(path),
            _ => {}
        }
        self
    }

    pub fn shortcut(mut self, hint: impl Into<SharedString>) -> Self {
        if let MenuItem::Action { shortcut, .. } = &mut self {
            *shortcut = Some(hint.into());
        }
        self
    }

    pub fn disabled(mut self, value: bool) -> Self {
        match &mut self {
            MenuItem::Action { disabled, .. } | MenuItem::Check { disabled, .. } => {
                *disabled = value
            }
            _ => {}
        }
        self
    }

    fn label(&self) -> Option<&SharedString> {
        match self {
            MenuItem::Action { label, .. }
            | MenuItem::Check { label, .. }
            | MenuItem::Submenu { label, .. } => Some(label),
            MenuItem::Separator => None,
        }
    }

    /// Separators and disabled items can't be highlighted.
    fn selectable(&self) -> bool {
        match self {
            MenuItem::Action { disabled, .. } | MenuItem::Check { disabled, .. } => !disabled,
            MenuItem::Submenu { items, .. } => !items.is_empty(),
            MenuItem::Separator => false,
        }
    }
}

struct OpenSubmenu {
    index: usize,
    menu: gpui::Entity<ContextMenu>,
    _subscription: Subscription,
}

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
}

impl EventEmitter<DismissEvent> for ContextMenu {}

impl Focusable for ContextMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ContextMenu {
    /// Creates a menu that takes keyboard focus as soon as it opens.
    pub fn build(
        items: Vec<MenuItem>,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Entity<ContextMenu> {
        Self::build_at_depth(items, 0, window, cx)
    }

    fn build_at_depth(
        items: Vec<MenuItem>,
        depth: usize,
        window: &mut Window,
        cx: &mut App,
    ) -> gpui::Entity<ContextMenu> {
        cx.new(|cx| {
            let focus_handle = cx.focus_handle();
            // Take focus once the event that opened the menu has finished: a click
            // focuses the clicked element's view after its handlers run.
            let handle = focus_handle.clone();
            window.defer(cx, move |window, cx| handle.focus(window, cx));
            theme::observe(window, cx);
            popover::close_on_deactivate(window, cx, |_, _, cx| cx.emit(DismissEvent));
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
                min_width: 220.,
            }
        })
    }

    /// Whether `position` is over this menu or any of its open submenus.
    pub fn contains(&self, position: &gpui::Point<gpui::Pixels>, cx: &App) -> bool {
        self.tracker.contains(position)
            || self
                .submenu
                .as_ref()
                .is_some_and(|s| s.menu.read(cx).contains(position, cx))
    }

    /// Starts with the first item highlighted, for menus opened from the keyboard.
    pub fn highlight_first(&mut self, cx: &mut Context<Self>) {
        self.move_highlight(Move::First, cx);
    }

    fn move_highlight(&mut self, movement: Move, cx: &mut Context<Self>) {
        let items = &self.items;
        self.highlighted = focus::step(
            self.highlighted,
            items.len(),
            movement,
            Layout::WRAPPING,
            |i| items[i].selectable(),
        );
        if let Some(index) = self.highlighted {
            self.scroll.scroll_to_item(index);
        }
        cx.notify();
    }

    fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        match &self.items[index] {
            MenuItem::Action {
                on_select,
                disabled: false,
                ..
            }
            | MenuItem::Check {
                on_select,
                disabled: false,
                ..
            } => {
                let on_select = on_select.clone();
                cx.emit(DismissEvent);
                // Run after the menu is gone, so the action can open dialogs or
                // move focus without the menu in the way.
                window.defer(cx, move |window, cx| on_select(window, cx));
            }
            MenuItem::Submenu { .. } => self.open_submenu(index, true, window, cx),
            _ => {}
        }
    }

    fn open_submenu(
        &mut self,
        index: usize,
        from_keyboard: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.submenu.as_ref().is_some_and(|s| s.index == index) {
            if from_keyboard && let Some(open) = &self.submenu {
                open.menu.update(cx, |menu, cx| {
                    menu.focus_handle.focus(window, cx);
                    menu.highlight_first(cx);
                });
            }
            return;
        }
        let MenuItem::Submenu { items, .. } = &mut self.items[index] else {
            return;
        };
        // Submenus are rebuilt each time they open, so take the items and put
        // them back when it closes.
        let items = std::mem::take(items);
        let menu = Self::build_at_depth(items, self.depth + 1, window, cx);
        if from_keyboard {
            menu.update(cx, |menu, cx| menu.highlight_first(cx));
        } else {
            // Hovering shouldn't steal the keyboard from this menu.
            self.focus_handle.focus(window, cx);
        }
        let subscription = cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, _, cx| {
            // An item in the submenu was chosen: close the whole chain.
            this.submenu = None;
            cx.emit(DismissEvent);
        });
        self.close_submenu(window, cx);
        self.submenu = Some(OpenSubmenu {
            index,
            menu,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Closes the open submenu (if any), returning its items to this menu.
    fn close_submenu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(open) = self.submenu.take() {
            let items = open
                .menu
                .update(cx, |menu, _| std::mem::take(&mut menu.items));
            if let MenuItem::Submenu { items: slot, .. } = &mut self.items[open.index] {
                *slot = items;
            }
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        if modifiers.control || modifiers.platform || modifiers.alt {
            return;
        }
        match key {
            "escape" => cx.emit(DismissEvent),
            "enter" | "space" => {
                if let Some(index) = self.highlighted {
                    self.activate(index, window, cx);
                }
            }
            "right" => {
                if let Some(index) = self.highlighted
                    && matches!(self.items[index], MenuItem::Submenu { .. })
                {
                    self.open_submenu(index, true, window, cx);
                }
            }
            _ => {
                if let Some(movement) = focus::group_key(event, Axis::Vertical) {
                    self.move_highlight(movement, cx);
                } else if let Some(ch) = event.keystroke.key_char.as_deref()
                    && !ch.trim().is_empty()
                {
                    let labels: Vec<&str> = self
                        .items
                        .iter()
                        .map(|item| item.label().map_or("", |l| l.as_ref()))
                        .collect();
                    let query = self.type_ahead.push(ch, Instant::now());
                    if let Some(index) =
                        super::dropdown::find_match(&labels, &query, self.highlighted, |i| {
                            self.items[i].selectable()
                        })
                    {
                        self.highlighted = Some(index);
                        self.scroll.scroll_to_item(index);
                        cx.notify();
                    }
                } else {
                    return;
                }
            }
        }
        cx.stop_propagation();
    }

    /// Left or Escape in a submenu closes just that submenu. Runs in the capture
    /// phase, before the submenu sees the key.
    fn on_key_down_capture_left(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event.keystroke.key.as_str(), "left" | "escape")
            && let Some(open) = &self.submenu
            && open.menu.read(cx).focus_handle.is_focused(window)
        {
            self.close_submenu(window, cx);
            cx.stop_propagation();
        }
    }
}

impl Render for ContextMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let open_submenu = self.submenu.as_ref().map(|s| s.index);
        let any_icons = self.items.iter().any(|item| {
            matches!(
                item,
                MenuItem::Action { icon: Some(_), .. } | MenuItem::Submenu { icon: Some(_), .. }
            )
        });

        let rows = self.items.iter().enumerate().map(|(i, item)| {
            let tracker = self.rows[i].clone();
            let highlighted = self.highlighted == Some(i) || open_submenu == Some(i);
            let row = match item {
                MenuItem::Separator => {
                    return div()
                        .id(i)
                        .my(px(4.))
                        .mx(px(8.))
                        .h(px(1.))
                        .bg(p.divider)
                        .into_any_element();
                }
                MenuItem::Action {
                    label,
                    icon,
                    shortcut,
                    disabled,
                    ..
                } => OptionRow::new("row", label.clone())
                    .icon(*icon)
                    .shortcut(shortcut.clone())
                    .disabled(*disabled)
                    .check_column(false),
                MenuItem::Check {
                    label,
                    checked,
                    disabled,
                    ..
                } => OptionRow::new("row", label.clone())
                    .checked(*checked)
                    .disabled(*disabled),
                MenuItem::Submenu { label, icon, items } => OptionRow::new("row", label.clone())
                    .icon(*icon)
                    .submenu(true)
                    .disabled(items.is_empty() && open_submenu != Some(i)),
            }
            .highlighted(highlighted)
            .reserve_icon(any_icons)
            .height(40.);
            let selectable = item.selectable() || open_submenu == Some(i);
            let is_submenu = matches!(item, MenuItem::Submenu { .. });

            div()
                .id(i)
                .relative()
                .child(tracker.element())
                .child(row)
                .when(selectable, |r| {
                    r.on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                        if *hovered {
                            this.highlighted = Some(i);
                            if is_submenu {
                                this.open_submenu(i, false, window, cx);
                            } else {
                                this.close_submenu(window, cx);
                            }
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |this, _, window, cx| this.activate(i, window, cx)))
                })
                .into_any_element()
        });

        let submenu = self.submenu.as_ref().and_then(|open| {
            let bounds = self.rows[open.index].bounds()?;
            Some(popover::attached(
                bounds,
                Side::Right,
                4.,
                self.depth + 2,
                open.menu.clone(),
            ))
        });

        let tracker = self.tracker.clone();
        // The submenu renders inside this element, so keys pressed in it pass
        // through here: Left and Escape are caught on the way down.
        div()
            .relative()
            .track_focus(&self.focus_handle)
            .key_context("Menu")
            .on_key_down(cx.listener(Self::on_key_down))
            .capture_key_down(cx.listener(Self::on_key_down_capture_left))
            .child(
                popover::surface(&p, window)
                    .id("menu")
                    .role(gpui::accesskit::Role::Menu)
                    // Only the root menu closes on outside clicks; it knows about the
                    // whole chain of submenus.
                    .when(self.depth == 0, |menu| {
                        menu.on_mouse_down_out(cx.listener(
                            |this, event: &gpui::MouseDownEvent, _, cx| {
                                if !this.contains(&event.position, cx) {
                                    cx.emit(DismissEvent);
                                }
                            },
                        ))
                    })
                    // Don't let a right click inside reopen the menu underneath.
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                    .min_w(px(self.min_width))
                    .max_w(px(420.))
                    .p(px(6.))
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .text_size(px(metrics::TEXT))
                    .child(tracker.element())
                    .children(rows),
            )
            .children(submenu)
    }
}
