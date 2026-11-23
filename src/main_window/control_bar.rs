//! The player's control bar (S2.2, `controls-default.png`): 54 px under the video, drawn from a
//! declarative layout (`layout.rs`) and fitted to the window (`fit.rs`), with what doesn't fit
//! in an overflow `…` menu.
//!
//! What it shows, in one row or two, comes from the player layout Customize Player edits
//! (`player_layout`, S8.7). Music mode (S8.1.2) draws its own row, with Back 15 s and Forward
//! 15 s and no Subtitles.
//!
//! Every button shows hover, pressed, keyboard focus and disabled states and a tooltip naming
//! its command and shortcut (S2.2.4). Previous and Next are disabled where they would do
//! nothing (the light theme mock's faded transport means that).

mod buttons;
pub(super) mod clock;
mod extra_buttons;
mod fit;
mod items;
mod layout;
mod names;
mod play_button;
mod readouts;
mod time_entry;

use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Div, Entity, Subscription, Window, div,
    prelude::*, px,
};

use self::fit::{ITEM_GAP, PADDING};
pub(super) use self::items::Place;
pub(super) use self::layout::{BUTTON, CHIP_WIDTH, ICON, LEVEL_GAP, SEEK_MIN};
use super::MainWindow;
use crate::assets::icons;
use crate::player_layout::{self, Item, Layout, Piece, Zone};
use crate::settings::{self, StoredLayout};
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover::{self, BoundsTracker, Side};

/// The height of one of the control bar's rows in the designs.
pub const CONTROL_BAR_HEIGHT: f32 = 54.;
/// The icon size of the overflow button.
const OVERFLOW_ICON: f32 = 20.;

/// The control bar's own state: its layout and the overflow menu.
pub struct ControlBar {
    /// What the bars show, as Customize Player (S8.7) left it.
    layout: Layout,
    /// The stored layout [`Self::layout`] was read from, to notice when it changes.
    stored: StoredLayout,
    /// What it shows in music mode (S8.1.2).
    music_row: Vec<Piece>,
    /// The overflow menu, while it's open.
    overflow: Option<(Entity<ContextMenu>, Subscription)>,
    overflow_button: BoundsTracker,
    /// The current time while it's being edited (S2.4.1).
    time_edit: Option<readouts::TimeEdit>,
}

impl Default for ControlBar {
    fn default() -> Self {
        Self {
            layout: Layout::designed(),
            stored: StoredLayout::default(),
            music_row: player_layout::music_row(),
            overflow: None,
            overflow_button: BoundsTracker::new(),
            time_edit: None,
        }
    }
}

impl ControlBar {
    /// Reads the player layout again if Customize Player saved another one.
    pub(super) fn sync_layout(&mut self, cx: &App) {
        let stored = &settings::get(cx).prefs.player_layout;
        if *stored != self.stored {
            self.layout = Layout::from_stored(stored);
            self.stored = stored.clone();
        }
    }

    /// The player layout the bars show.
    pub(super) fn layout(&self) -> &Layout {
        &self.layout
    }

    /// Closes the overflow menu: another popover opens.
    pub(super) fn close_overflow(&mut self) {
        self.overflow = None;
    }

    /// Whether the overflow menu is open.
    pub(super) fn is_overflow_open(&self) -> bool {
        self.overflow.is_some()
    }

    /// Whether the current time is being typed (S2.4.1), which keeps the bar up.
    pub(super) fn is_editing_time(&self) -> bool {
        self.time_edit.is_some()
    }
}

impl MainWindow {
    /// The control bar's rows, top to bottom.
    fn bar_rows(&self) -> &[Vec<Piece>] {
        if self.is_customizing() {
            &self.shown_layout().rows
        } else if self.is_music_mode() {
            std::slice::from_ref(&self.control_bar.music_row)
        } else {
            &self.control_bar.layout.rows
        }
    }

    /// The control bar's height: a row's for each of its rows.
    pub(super) fn control_bar_height(&self) -> f32 {
        self.bar_rows().len().max(1) as f32 * CONTROL_BAR_HEIGHT
    }

    /// The control bar along the bottom of the window.
    pub(super) fn render_control_bar(
        &self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        if self.is_customizing() {
            return self.render_edit_bar(p, window, cx);
        }
        let width = f32::from(window.viewport_size().width);
        let fitted: Vec<fit::Fitted> =
            self.bar_rows().iter().map(|row| fit::fit(row, width)).collect();
        // One overflow menu holds what every row gave up; its button ends the last row.
        let overflow: Vec<Item> = fitted.iter().flat_map(|row| row.overflow.clone()).collect();
        let last = fitted.len().saturating_sub(1);
        let rows = fitted.iter().enumerate().map(|(index, row)| {
            let row = self.render_bar_row(&row.slots, p, cx);
            if index == last && !overflow.is_empty() {
                row.child(self.render_overflow_button(overflow.clone(), cx))
            } else {
                row
            }
        });
        bar_frame(self.control_bar_height(), p).children(rows.collect::<Vec<_>>())
    }

    /// The bar in Customize Player's edit mode: every row's pieces outlined, none fitted away.
    fn render_edit_bar(&self, p: &Palette, window: &Window, cx: &mut Context<Self>) -> Div {
        let rows = (0..self.bar_rows().len()).map(|row| {
            let zone = Zone::Row(row);
            let pieces = self.render_edit_pieces(zone, p, window, cx);
            let area = bar_row().debug_selector(move || format!("customize-row-{row}"));
            Self::edit_drop_zone(area.overflow_hidden(), zone, cx).children(pieces)
        });
        bar_frame(self.control_bar_height(), p).children(rows.collect::<Vec<_>>())
    }

    /// One of the bar's rows, drawing `pieces` left to right.
    fn render_bar_row(&self, pieces: &[Piece], p: &Palette, cx: &mut Context<Self>) -> Div {
        bar_row().children(pieces.iter().map(|piece| match piece {
            Piece::Item(item) => self.render_item(*item, Place::ControlBar, p, cx),
            Piece::Gap(width) => div().w(px(*width)).flex_none().into_any_element(),
            Piece::Spacer => div().flex_1().into_any_element(),
            Piece::Divider => divider(p).into_any_element(),
        }))
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

/// The bar along the window's bottom, `height` tall, its rows stacked.
fn bar_frame(height: f32, p: &Palette) -> Div {
    div()
        .absolute()
        .bottom_0()
        .left_0()
        .right_0()
        .h(px(height))
        .flex()
        .flex_col()
        .bg(p.control_bar)
        .occlude()
}

/// An empty row of the bar.
fn bar_row() -> Div {
    div()
        .h(px(CONTROL_BAR_HEIGHT))
        .flex_none()
        .px(px(PADDING))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(ITEM_GAP))
}

/// How wide `item` is in a bar, at least.
pub(super) fn item_width(item: Item) -> f32 {
    layout::fit(item).width
}

/// A divider between two groups of a bar.
pub(super) fn divider(p: &Palette) -> Div {
    div().w(px(layout::DIVIDER_WIDTH)).h(px(16.)).flex_none().bg(p.divider)
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;
    use crate::main_window::tests::{main_view, open};

    fn store_layout(cx: &mut gpui::VisualTestContext, title_bar: &[&str], rows: &[&[&str]]) {
        let ids = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect();
        let stored = StoredLayout {
            title_bar: ids(title_bar),
            // None stored: the designed window controls.
            window_controls: Vec::new(),
            controls: rows.iter().map(|row| ids(row)).collect(),
        };
        cx.update(|_, cx| settings::update(cx, |s| s.prefs.player_layout = stored));
        cx.run_until_parked();
    }

    #[gpui::test]
    fn a_saved_layout_reaches_the_bars_and_a_second_row_doubles_the_bar(cx: &mut TestAppContext) {
        let cx = open(cx);
        let main = main_view(cx);
        assert_eq!(main.read_with(cx, |main, _| main.control_bar_height()), CONTROL_BAR_HEIGHT);

        store_layout(cx, &["menu", "title"], &[&["timeline"], &["spacer", "play-pause", "stop"]]);
        main.read_with(cx, |main, _| {
            assert_eq!(main.control_bar.layout().rows.len(), 2);
            assert_eq!(main.control_bar_height(), 2. * CONTROL_BAR_HEIGHT);
            assert_eq!(main.control_bar.layout().title_bar.len(), 2);
        });
        let stop = main.read_with(cx, |main, cx| main.button_item(Item::Stop, cx).map(|b| b.label));
        assert_eq!(stop, Some("Stop playback"));

        store_layout(cx, &[], &[]);
        main.read_with(cx, |main, _| assert_eq!(*main.control_bar.layout(), Layout::designed()));
    }
}
