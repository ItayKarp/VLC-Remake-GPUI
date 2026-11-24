//! The Queue tab's header (S3.2.2): "N items - Xh Ym", then sort, repeat, shuffle, "−" (remove
//! the selected rows) and "+" (add files). Sort opens a menu of one-time sorts (S3.2.5). Repeat
//! cycles Off → All → One and Shuffle switches on and off (S3.3), as their commands do.

use gpui::{AnyElement, ClickEvent, Context, DismissEvent, Div, Window, div, prelude::*, px};

use super::MainWindow;
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::keymap::actions as act;
use crate::main_window::play_order::repeat_button_look;
use crate::queue::SortKey;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::IconButton;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, Side};

/// The header's height, its text size, and its buttons' and icons' sizes.
const HEIGHT: f32 = 52.;
const TEXT: f32 = 14.;
const BUTTON: f32 = 32.;
const ICON: f32 = 20.;
/// The header's side padding (the design's 28 px to the text, less the buttons' inset).
const PADDING_LEFT: f32 = 28.;
const PADDING_RIGHT: f32 = 16.;
/// The space between the buttons.
const BUTTON_GAP: f32 = 4.;
/// How far below its button the sort menu opens.
const MENU_GAP: f32 = 4.;

impl MainWindow {
    /// The header above the rows.
    pub(super) fn render_queue_header(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let total = self.queue_panel.lengths.total(self.queue.entries()).label();
        let sort = self.sort_button(cx);
        let remove = IconButton::new("queue-remove", icons::REMOVE, tr!("Remove selected"))
            .size(BUTTON, ICON)
            .disabled(self.queue_panel.selection.is_empty())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.remove_selected(window, cx);
            }));
        let add = IconButton::new("queue-add", icons::ADD, tr!("Add files"))
            .size(BUTTON, ICON)
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                Self::prompt_to_enqueue(window, cx);
            }));
        let buttons = div()
            .flex_row_start()
            .flex_none()
            .gap(px(BUTTON_GAP))
            .child(sort)
            .children(self.play_order_buttons(cx))
            .child(remove);
        div()
            .flex_none()
            .h(px(HEIGHT))
            .flex_row_start()
            .items_center()
            .justify_between()
            .ps(px(PADDING_LEFT))
            .pe(px(PADDING_RIGHT))
            .text_size(text_px(TEXT))
            .text_color(p.text_muted)
            .child(div().min_w_0().truncate().child(total))
            .child(buttons.child(add))
    }

    /// Sort, which opens the sort menu; there's nothing to sort below two entries.
    fn sort_button(&self, cx: &mut Context<Self>) -> Div {
        let sort = IconButton::new("queue-sort", icons::SORT, tr!("Sort"))
            .size(BUTTON, ICON)
            .selected(self.queue_panel.sort_menu.is_some())
            .disabled(self.queue.len() < 2)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_sort_menu(window, cx);
            }));
        div().relative().flex_none().child(self.queue_panel.sort_button.element()).child(sort)
    }

    /// Repeat and Shuffle, which run their commands.
    fn play_order_buttons(&self, cx: &mut Context<Self>) -> [IconButton; 2] {
        let (repeat_icon, repeating, repeat_label) = repeat_button_look(self.queue.repeat());
        let repeat = IconButton::new("queue-repeat", repeat_icon, repeat_label)
            .size(BUTTON, ICON)
            .selected(repeating)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.perform(&act::CycleRepeat, window, cx);
            }));
        let shuffled = self.queue.is_shuffled();
        let shuffle_label = if shuffled { tr!("Shuffle: On") } else { tr!("Shuffle: Off") };
        let shuffle = IconButton::new("queue-shuffle", icons::SHUFFLE, shuffle_label)
            .size(BUTTON, ICON)
            .selected(shuffled)
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.perform(&act::ToggleShuffle, window, cx);
            }));
        [repeat, shuffle]
    }

    /// Opens the sort menu below its button, or closes it.
    fn toggle_sort_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queue_panel.sort_menu.take().is_some() {
            cx.notify();
            return;
        }
        let main = cx.entity().downgrade();
        let items = SortKey::ALL.iter().map(|&key| {
            let main = main.clone();
            MenuItem::action(key.label(), move |_, cx| {
                main.update(cx, |main, cx| main.sort_queue(key, cx)).ok();
            })
        });
        let menu = ContextMenu::build(items.collect(), window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.queue_panel.sort_menu = None;
            cx.notify();
        });
        self.queue_panel.sort_menu = Some((menu, subscription));
        cx.notify();
    }

    /// The sort menu, while open.
    pub(in crate::main_window) fn render_sort_menu(&self) -> Option<AnyElement> {
        let (menu, _) = self.queue_panel.sort_menu.as_ref()?;
        let button = self.queue_panel.sort_button.bounds()?;
        Some(popover::attached(button, Side::Below, MENU_GAP, 1, menu.clone()))
    }
}
