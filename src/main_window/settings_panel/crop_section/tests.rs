//! The Crop section: pixels typed for each edge, linked pairs, and the most an edge can take
//! of the video playing.

use gpui::{Focusable, TestAppContext, VisualTestContext};

use super::*;
use crate::engine::backend::BackendEvent;
use crate::engine::media_info::VideoSize;
use crate::main_window::now_playing::tests::{Playing, draw};
use crate::main_window::settings_panel::PanelTab;
use crate::main_window::settings_panel::tests::{expand, field_text, open_tab, press_near, submit};
use crate::main_window::tests::main_view;

/// The Video tab with the Crop section open.
fn open_crop(cx: &mut TestAppContext) -> (&mut VisualTestContext, Playing) {
    let (cx, playing) = open_tab(cx, PanelTab::Video);
    expand(cx, &[VIDEO_CROP]);
    (cx, playing)
}

fn field(cx: &mut VisualTestContext, edge: Edge) -> Entity<TextInput> {
    main_view(cx).read_with(cx, |main, _| main.settings_panel.crop.field(edge).clone())
}

fn edges(cx: &mut VisualTestContext) -> CropEdges {
    cx.update(|_, cx| effects::effective(cx).prefs.video.crop_edges)
}

fn link(cx: &mut VisualTestContext, edge: Edge) {
    main_view(cx).update(cx, |main, cx| main.toggle_crop_link(edge, cx));
    draw(cx);
}

#[test]
fn pixels_read_with_or_without_the_unit() {
    assert_eq!(parse_pixels("12"), Some(12));
    assert_eq!(parse_pixels(" 12 PX "), Some(12));
    assert_eq!(parse_pixels(""), Some(0));
    assert_eq!(parse_pixels("-3"), None);
    assert_eq!(parse_pixels("a lot"), None);
}

#[test]
fn an_edge_takes_what_its_opposite_leaves() {
    assert_eq!(most_for(1064, 64, false), 1000);
    assert_eq!(most_for(1064, 2000, false), 0);
    assert_eq!(most_for(1064, 64, true), 532, "linked, both take it");
}

#[test]
fn edges_follow_the_sections_order() {
    assert_eq!(Edge::Top.key(), "video.crop_edges.top");
    assert_eq!(Edge::Right.key(), "video.crop_edges.right");
    assert_eq!(Edge::Left.opposite(), Edge::Right);
    assert_eq!(Edge::Bottom.pair(), 0);
}

#[gpui::test]
fn typed_pixels_crop_their_edge(cx: &mut TestAppContext) {
    let (cx, _playing) = open_crop(cx);
    let top = field(cx, Edge::Top);
    submit(cx, &top, "12 px");
    assert_eq!(edges(cx).top, 12);
    assert_eq!(edges(cx).bottom, 0);
    let bottom = field(cx, Edge::Bottom);
    assert_eq!(field_text(cx, &bottom), ("0".into(), false));
    submit(cx, &top, "a lot");
    assert_eq!(field_text(cx, &top), ("a lot".into(), true));
    assert_eq!(edges(cx).top, 12, "nothing changes");
}

#[gpui::test]
fn a_linked_pair_moves_together(cx: &mut TestAppContext) {
    let (cx, _playing) = open_crop(cx);
    let edge = field(cx, Edge::Left);
    submit(cx, &edge, "8");
    link(cx, Edge::Left);
    assert_eq!((edges(cx).left, edges(cx).right), (8, 8), "linking evens the pair");
    let edge = field(cx, Edge::Right);
    submit(cx, &edge, "20");
    assert_eq!((edges(cx).left, edges(cx).right), (20, 20));
    link(cx, Edge::Right);
    let edge = field(cx, Edge::Left);
    submit(cx, &edge, "4");
    assert_eq!((edges(cx).left, edges(cx).right), (4, 20), "unlinked, each edge is its own");
}

#[gpui::test]
fn an_edge_leaves_some_of_the_picture(cx: &mut TestAppContext) {
    let (cx, playing) = open_crop(cx);
    let size = VideoSize { width: 640, height: 360 };
    playing.mock.emit(BackendEvent::VideoSize(Some(size)));
    draw(cx);
    let top = field(cx, Edge::Top);
    submit(cx, &top, "400");
    let (_, invalid) = field_text(cx, &top);
    assert!(invalid);
    assert_eq!(edges(cx).top, 0);
    submit(cx, &top, "300");
    assert_eq!(edges(cx).top, 300);
}

#[gpui::test]
fn the_link_button_answers_the_keyboard(cx: &mut TestAppContext) {
    let (cx, _playing) = open_crop(cx);
    let left = field(cx, Edge::Left);
    submit(cx, &left, "6");
    let right = field(cx, Edge::Right);
    let handle = right.read_with(cx, Focusable::focus_handle);
    press_near(cx, &handle, 1, "space");
    assert_eq!((edges(cx).left, edges(cx).right), (6, 6), "linking takes the first edge's");
}
