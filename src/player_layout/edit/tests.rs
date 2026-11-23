use super::*;

fn spot(zone: Zone, index: usize) -> Spot {
    Spot { zone, index }
}

fn row(layout: &Layout, row: usize) -> Vec<Piece> {
    layout.pieces(Zone::Row(row)).to_vec()
}

#[test]
fn the_designed_palette_matches_the_mock() {
    let palette = Layout::designed().palette();
    assert_eq!(palette[..3], [Unit::Spacer, Unit::Divider, Unit::Item(Item::Speed)]);
    assert!(!palette.contains(&Unit::Item(Item::PlayPause)));
    assert!(palette.contains(&Unit::Item(Item::Playlists)));
    // Spacer, Divider and the 29 items of `player_customize_01.png`.
    assert_eq!(palette.len(), 31);
    assert_eq!(palette[30], Unit::Item(Item::Playlists));
}

#[test]
fn the_timeline_moves_as_one_into_a_new_row() {
    let mut layout = Layout::designed();
    // Pressing the seek bar picks up the whole timeline.
    layout.move_unit(spot(Zone::Row(0), 5), Target::NewRow);
    assert_eq!(
        row(&layout, 0),
        [Item::CurrentTime, Item::SeekBar, Item::Duration].map(Piece::Item)
    );
    assert!(!row(&layout, 1).contains(&Piece::Item(Item::SeekBar)));
}

#[test]
fn moving_right_within_a_row_lands_before_the_target() {
    let mut layout = Layout::designed();
    // Previous, Play/Pause, Next: Previous dropped before Next goes between the two.
    layout.move_unit(spot(Zone::Row(0), 0), Target::Into(spot(Zone::Row(0), 2)));
    assert_eq!(
        row(&layout, 0)[..3],
        [Item::PlayPause, Item::Previous, Item::Next].map(Piece::Item)
    );
}

#[test]
fn moving_onto_itself_changes_nothing() {
    let mut layout = Layout::designed();
    layout.move_unit(spot(Zone::Row(0), 4), Target::Into(spot(Zone::Row(0), 6)));
    assert_eq!(layout, Layout::designed());
}

#[test]
fn items_are_added_once_but_spacers_any_number_of_times() {
    let mut layout = Layout::designed();
    let end = Target::Into(spot(Zone::TitleBar, 99));
    layout.add(Unit::Item(Item::Stop), end);
    layout.add(Unit::Item(Item::Stop), end);
    layout.add(Unit::Spacer, end);
    layout.add(Unit::Spacer, end);
    let title_bar = layout.pieces(Zone::TitleBar);
    assert_eq!(title_bar.iter().filter(|p| **p == Piece::Item(Item::Stop)).count(), 1);
    assert_eq!(title_bar.iter().filter(|p| **p == Piece::Spacer).count(), 2);
    assert!(!layout.palette().contains(&Unit::Item(Item::Stop)));
}

#[test]
fn removing_puts_an_item_back_in_the_palette() {
    let mut layout = Layout::designed();
    layout.remove(spot(Zone::TitleBar, 0));
    assert!(!layout.is_placed(Item::Menu));
    assert!(layout.palette().contains(&Unit::Item(Item::Menu)));
}

#[test]
fn a_row_left_empty_goes_but_the_last_one_stays() {
    let mut layout = Layout::designed();
    layout.move_unit(spot(Zone::Row(0), 4), Target::NewRow);
    layout.remove(spot(Zone::Row(0), 0));
    assert_eq!(layout.rows.len(), 1);
    while !layout.rows[0].is_empty() {
        let Some(index) =
            (0..layout.rows[0].len()).find(|i| layout.unit_at(spot(Zone::Row(0), *i)).is_some())
        else {
            break;
        };
        layout.remove(spot(Zone::Row(0), index));
    }
    assert_eq!(layout.rows.len(), 1);
}

#[test]
fn a_third_row_isnt_made() {
    let mut layout = Layout::designed();
    layout.move_unit(spot(Zone::Row(0), 0), Target::NewRow);
    layout.add(Unit::Item(Item::Stop), Target::NewRow);
    assert_eq!(layout.rows.len(), MAX_ROWS);
    assert_eq!(row(&layout, 0)[0], Piece::Item(Item::Stop));
}

#[test]
fn gaps_arent_units() {
    let layout = Layout::designed();
    assert_eq!(layout.unit_at(spot(Zone::Row(0), 3)), None);
}
