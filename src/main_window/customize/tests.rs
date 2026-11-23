//! Customize Player driven as a person does: the command, drags, Save and Cancel.

use gpui::{Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px};

use crate::keymap::actions::CustomizePlayer;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open, tap};
use crate::player_layout::{Item, Layout, Piece, Zone};
use crate::settings::{self, StoredLayout};

fn customize(cx: &mut TestAppContext) -> &mut VisualTestContext {
    let cx = open(cx);
    cx.dispatch_action(CustomizePlayer);
    cx.run_until_parked();
    assert!(main_view(cx).read_with(cx, |main, _| main.is_customizing()));
    cx
}

fn center(cx: &mut VisualTestContext, selector: &'static str) -> Point<gpui::Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center()
}

/// Drags from `from` to `to`, as a pointer does.
fn drag(cx: &mut VisualTestContext, from: Point<gpui::Pixels>, to: Point<gpui::Pixels>) {
    let none = Modifiers::none();
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.simulate_mouse_move(point(from.x + px(10.), from.y + px(10.)), MouseButton::Left, none);
    cx.run_until_parked();
    cx.simulate_mouse_move(to, MouseButton::Left, none);
    cx.run_until_parked();
    cx.simulate_mouse_move(point(to.x + px(1.), to.y), MouseButton::Left, none);
    cx.simulate_mouse_up(to, MouseButton::Left, none);
    cx.run_until_parked();
}

fn draft(cx: &mut VisualTestContext) -> Layout {
    main_view(cx).read_with(cx, |main, _| main.shown_layout().clone())
}

fn stored(cx: &mut VisualTestContext) -> StoredLayout {
    cx.update(|_, cx| settings::get(cx).prefs.player_layout.clone())
}

#[gpui::test]
fn an_item_dragged_from_the_palette_joins_the_bar_and_save_keeps_it(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let stop = center(cx, "palette-Stop");
    let row = cx.debug_bounds("customize-row-0").expect("the bar is drawn");
    // The bar's right end, past every piece.
    drag(cx, stop, point(row.right() - px(5.), row.center().y));
    assert!(draft(cx).pieces(Zone::Row(0)).contains(&Piece::Item(Item::Stop)));
    assert_eq!(stored(cx), StoredLayout::default(), "nothing is saved before Save");

    main_view(cx).update_in(cx, MainWindow::save_layout);
    assert!(Layout::from_stored(&stored(cx)).is_placed(Item::Stop));
    assert!(!main_view(cx).read_with(cx, |main, _| main.is_customizing()));
}

#[gpui::test]
fn a_piece_dragged_into_the_palette_leaves_the_bars(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let queue = center(cx, "customize-Queue");
    let palette = center(cx, "customize-palette");
    drag(cx, queue, palette);
    assert!(!draft(cx).is_placed(Item::List));
}

#[gpui::test]
fn escape_cancels_and_keeps_the_saved_layout(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let queue = center(cx, "customize-Queue");
    let palette = center(cx, "customize-palette");
    drag(cx, queue, palette);
    tap(cx, "escape");
    assert!(!main_view(cx).read_with(cx, |main, _| main.is_customizing()));
    assert_eq!(draft(cx), Layout::designed());
    assert_eq!(stored(cx), StoredLayout::default());
}

#[gpui::test]
fn reset_to_default_puts_the_designed_layout_back(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let queue = center(cx, "customize-Queue");
    let palette = center(cx, "customize-palette");
    drag(cx, queue, palette);
    main_view(cx).update(cx, MainWindow::reset_layout);
    assert_eq!(draft(cx), Layout::designed());
}

#[gpui::test]
fn a_title_bar_item_moves_into_the_control_bar(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let menu = center(cx, "customize-Menu");
    let row = cx.debug_bounds("customize-row-0").expect("the bar is drawn");
    drag(cx, menu, point(row.left() + px(2.), row.center().y));
    let layout = draft(cx);
    assert_eq!(layout.pieces(Zone::Row(0)).first(), Some(&Piece::Item(Item::Menu)));
    assert!(!layout.pieces(Zone::TitleBar).contains(&Piece::Item(Item::Menu)));
}
