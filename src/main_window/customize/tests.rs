//! Customize Player driven as a person does: the command, drags, Save and Cancel.

use gpui::{Modifiers, MouseButton, Point, TestAppContext, VisualTestContext, point, px};

use crate::keymap::actions::CustomizePlayer;
use crate::main_window::MainWindow;
use crate::main_window::tests::{main_view, open, tap};
use crate::player_layout::{Item, Layout, Piece, Spot, Step, Unit, Zone};
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

#[gpui::test]
fn fullscreen_is_pulled_out_of_the_window_controls(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let fullscreen = center(cx, "customize-Fullscreen");
    let row = cx.debug_bounds("customize-row-0").expect("the bar is drawn");
    drag(cx, fullscreen, point(row.right() - px(5.), row.center().y));
    let layout = draft(cx);
    assert_eq!(layout.pieces(Zone::Row(0)).last(), Some(&Piece::Item(Item::Fullscreen)));
    let stays = [Item::Minimize, Item::Maximize, Item::Close].map(Piece::Item);
    assert_eq!(layout.pieces(Zone::WindowControls), stays);
}

#[gpui::test]
fn an_item_dropped_among_the_window_controls_joins_them(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let stop = center(cx, "palette-Stop");
    let controls = cx.debug_bounds("customize-window-controls").expect("they are drawn");
    drag(cx, stop, point(controls.left() + px(2.), controls.center().y));
    // Before the first one drawn: Fullscreen on macOS, where the traffic lights stand in for
    // the others, Minimize elsewhere.
    let controls = draft(cx).pieces(Zone::WindowControls).to_vec();
    let stop = controls.iter().position(|piece| *piece == Piece::Item(Item::Stop));
    let fullscreen = controls.iter().position(|piece| *piece == Piece::Item(Item::Fullscreen));
    let expected = if cfg!(target_os = "macos") { fullscreen.map(|at| at - 1) } else { Some(0) };
    assert_eq!(stop, expected);
}

#[gpui::test]
fn the_list_moves_removes_and_adds_without_a_pointer(cx: &mut TestAppContext) {
    let cx = customize(cx);
    main_view(cx).update(cx, MainWindow::toggle_list);
    cx.run_until_parked();
    assert!(cx.debug_bounds("customize-list").is_some(), "the list replaces the units");
    let previous = Spot { zone: Zone::Row(0), index: 0 };
    main_view(cx).update(cx, |main, cx| main.nudge_unit(previous, Step::Later, cx));
    assert_eq!(draft(cx).pieces(Zone::Row(0))[1], Piece::Item(Item::Previous));
    main_view(cx).update_in(cx, |main, window, cx| main.remove_listed(previous, window, cx));
    assert!(!draft(cx).is_placed(Item::PlayPause));
    main_view(cx).update_in(cx, |main, window, cx| {
        main.add_listed(Unit::Item(Item::Stop), window, cx);
    });
    assert_eq!(draft(cx).pieces(Zone::Row(0)).last(), Some(&Piece::Item(Item::Stop)));
}

#[gpui::test]
fn an_item_dropped_on_a_piece_lands_beside_it(cx: &mut TestAppContext) {
    let cx = customize(cx);
    let stop = center(cx, "palette-Stop");
    let volume = center(cx, "customize-Volume");
    // Just right of the volume's middle: after it, before Audio (the group gap between
    // them stays put).
    drag(cx, stop, point(volume.x + px(4.), volume.y));
    let row = draft(cx).pieces(Zone::Row(0)).to_vec();
    let audio = row.iter().position(|piece| *piece == Piece::Item(Item::Audio));
    assert_eq!(audio.map(|at| row[at - 1]), Some(Piece::Item(Item::Stop)));
    // And on a title bar piece: just left of the menu's middle, before it.
    let snapshot = center(cx, "palette-Snapshot");
    let menu = center(cx, "customize-Menu");
    drag(cx, snapshot, point(menu.x - px(4.), menu.y));
    assert_eq!(draft(cx).pieces(Zone::TitleBar).first(), Some(&Piece::Item(Item::Snapshot)));
}
