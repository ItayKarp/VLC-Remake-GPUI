//! The tray menu's entries, ids and clicks, and the icon's pixels, as the StatusNotifierItem
//! serves them.

use std::io::Cursor;

use super::*;
use crate::tray::ICON_PNG;

#[test]
fn the_menu_lists_the_groups_with_a_separator_between() {
    let entries: Vec<_> = menu_ids().into_iter().map(Entry::from_id).collect();
    let command = |command| Some(Entry::Command(command));
    let expected = [
        command(TrayCommand::PlayPause),
        command(TrayCommand::Previous),
        command(TrayCommand::Next),
        Some(Entry::Separator),
        command(TrayCommand::ShowVlc),
        command(TrayCommand::Quit),
    ];
    assert_eq!(entries, expected);
}

#[test]
fn ids_are_unique_and_never_the_root() {
    let ids = menu_ids();
    assert!(!ids.contains(&ROOT_ID));
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), ids.len());
}

#[test]
fn ids_the_menu_lacks_are_no_entry() {
    assert_eq!(Entry::from_id(ROOT_ID), Some(Entry::Root));
    for id in [-1, 6, SEPARATOR_IDS, SEPARATOR_IDS + 2, 999] {
        assert_eq!(Entry::from_id(id), None, "{id}");
    }
}

#[test]
fn each_entry_has_its_one_property() {
    assert_eq!(Entry::Root.property(), ("children-display", "submenu"));
    assert_eq!(Entry::Separator.property(), ("type", "separator"));
    let quit = TrayCommand::Quit;
    assert_eq!(Entry::Command(quit).property(), ("label", quit.label()));
}

#[test]
fn only_a_click_on_a_command_runs_it() {
    let next = TrayCommand::Next.id();
    assert_eq!(chosen(next, "clicked"), Some(TrayCommand::Next));
    assert_eq!(chosen(next, "hovered"), None);
    assert_eq!(chosen(ROOT_ID, "clicked"), None);
    assert_eq!(chosen(separator_id(1), "clicked"), None);
    assert_eq!(chosen(42, "clicked"), None);
}

#[test]
fn pixels_turn_from_rgba_to_argb() {
    let mut png = Vec::new();
    let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([10, 20, 30, 128]));
    image.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let (width, height, pixels) = pixmap(&png).unwrap();
    assert_eq!((width, height), (2, 1));
    assert_eq!(pixels, [128, 10, 20, 30, 128, 10, 20, 30]);
}

#[test]
fn the_cone_is_a_square_pixmap() {
    let (width, height, pixels) = pixmap(ICON_PNG).unwrap();
    assert_eq!(width, height);
    assert_eq!(pixels.len(), (width * height * 4) as usize);
}

#[test]
fn bytes_that_arent_a_png_are_an_error() {
    assert!(pixmap(b"not a png").is_err());
}
