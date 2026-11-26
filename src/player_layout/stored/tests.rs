//! Reading and writing stored layouts.

use super::*;

/// The designed window controls' ids.
const CONTROLS: &[&str] = &["minimize", "maximize", "fullscreen", "close"];

fn stored(title_bar: &[&str], rows: &[&[&str]]) -> StoredLayout {
    stored_with(title_bar, CONTROLS, rows)
}

fn stored_with(title_bar: &[&str], window_controls: &[&str], rows: &[&[&str]]) -> StoredLayout {
    let ids = |ids: &[&str]| ids.iter().map(|id| (*id).to_owned()).collect();
    StoredLayout {
        title_bar: ids(title_bar),
        window_controls: ids(window_controls),
        controls: rows.iter().map(|row| ids(row)).collect(),
    }
}

fn items(items: &[Item]) -> Vec<Piece> {
    items.iter().copied().map(Piece::Item).collect()
}

#[test]
fn nothing_stored_is_the_designed_layout_and_it_stores_as_nothing() {
    assert_eq!(parse(&StoredLayout::default()), Layout::designed());
    assert_eq!(write(&Layout::designed()), StoredLayout::default());
}

#[test]
fn a_customized_layout_survives_a_round_trip() {
    let original = stored(
        &["menu", "sidebar"],
        &[&["timeline"], &["spacer", "previous", "play-pause", "next", "spacer", "gap-8", "queue"]],
    );
    let layout = parse(&original);
    assert_eq!(layout.rows[0], [Item::CurrentTime, Item::SeekBar, Item::Duration].map(Piece::Item));
    assert_eq!(write(&layout), original);
}

#[test]
fn every_item_has_an_id_and_round_trips() {
    for (item, id) in IDS {
        assert_eq!(pieces_of(id), Some(vec![Piece::Item(*item)]), "{id}");
        assert_eq!(id_of(Piece::Item(*item)).as_deref(), Some(*id));
    }
}

#[test]
fn a_second_copy_of_an_item_is_dropped() {
    let layout = parse(&stored(&["menu", "timeline"], &[&["menu", "timeline", "stop", "stop"]]));
    assert_eq!(layout.title_bar.len(), 4);
    assert_eq!(layout.rows[0], [Piece::Item(Item::Stop)]);
}

#[test]
fn unknown_ids_are_skipped_and_saved_again_where_they_were() {
    let original = stored(&["menu", "cast"], &[&["play-pause", "hologram", "queue"]]);
    let layout = parse(&original);
    assert_eq!(layout.title_bar, [Piece::Item(Item::Menu)]);
    assert_eq!(layout.rows[0], [Piece::Item(Item::PlayPause), Piece::Item(Item::List)]);
    assert_eq!(write(&layout), original);
}

#[test]
fn bad_gaps_and_extra_rows_are_ignored() {
    let layout = parse(&stored(&[], &[&["gap-x", "gap-500", "gap-12"], &["stop"], &["quit"]]));
    assert_eq!(layout.rows, [vec![Piece::Gap(12.)], vec![Piece::Item(Item::Stop)]]);
}

#[test]
fn a_layout_saved_before_the_window_controls_gets_the_designed_ones() {
    let layout = parse(&stored_with(&["menu"], &[], &[&["play-pause"]]));
    assert_eq!(layout.window_controls, Layout::designed().window_controls);
    // Fullscreen already moved stays where it is.
    let layout = parse(&stored_with(&["menu"], &[], &[&["play-pause", "fullscreen"]]));
    assert_eq!(layout.window_controls, items(&[Item::Minimize, Item::Maximize, Item::Close]));
    assert_eq!(layout.rows[0], items(&[Item::PlayPause, Item::Fullscreen]));
}

#[test]
fn fixed_window_controls_stay_among_the_window_controls() {
    let layout = parse(&stored_with(&["close"], &["stop", "maximize"], &[&["minimize", "next"]]));
    assert_eq!(layout.title_bar, Vec::<Piece>::new());
    assert_eq!(layout.rows[0], items(&[Item::Next]));
    assert_eq!(
        layout.window_controls,
        items(&[Item::Stop, Item::Maximize, Item::Minimize, Item::Close])
    );
}
