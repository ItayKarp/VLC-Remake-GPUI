//! Home's page: where its items are, how the keyboard walks them, and where its folders open.

use gpui::{TestAppContext, VisualTestContext};

use super::*;
use crate::db::media::MediaId;
use crate::db::metadata::{MediaKind, Metadata};
use crate::library::watch_state::WatchState;
use crate::main_window::library_panel::listing::Body;
use crate::main_window::library_panel::selection::Key;
use crate::main_window::tests::{main_view, open, tap};

fn card(id: i64) -> Card {
    Card {
        label: format!("E{id:02}.mkv"),
        path: PathBuf::from(format!("/v/E{id:02}.mkv")),
        media: Some(MediaId(id)),
        kind: MediaKind::Video,
        watch: WatchState::InProgress(0.5),
        tags: Metadata::default(),
        added_ms: None,
    }
}

/// The main window with the sidebar open on Home, showing `home`.
fn home_with(cx: &mut TestAppContext, home: Home) -> &mut VisualTestContext {
    let cx = open(cx);
    let main = main_view(cx);
    main.update_in(cx, |main, window, cx| {
        main.navigate(Route::Home, cx);
        main.toggle_library_panel(window, cx);
    });
    cx.run_until_parked();
    main.update(cx, |main, cx| main.home_read(home, true, cx));
    cx.run_until_parked();
    cx
}

fn keys(cx: &mut VisualTestContext) -> Vec<Key> {
    let main = main_view(cx);
    cx.update(|_, cx| {
        let Body::Items(items) = main.read(cx).listing(cx) else { panic!("Home was read") };
        items.keys()
    })
}

#[gpui::test]
fn nothing_yet_lists_nothing(cx: &mut TestAppContext) {
    let cx = home_with(cx, Home::default());
    assert_eq!(keys(cx), []);
}

#[gpui::test]
fn the_page_lists_media_first_and_the_keyboard_follows_it(cx: &mut TestAppContext) {
    let home = Home {
        continue_watching: (1..=5).map(card).collect(),
        artists: vec!["juxtopposed".into()],
        ..Home::default()
    };
    let cx = home_with(cx, home);
    let artist = Key::Place(Route::Music(MusicRoute::Artists(Some("juxtopposed".into()))));
    let shown = keys(cx);
    // Three of the five partway videos, then the artist.
    assert_eq!(shown[..3], [1, 2, 3].map(|id| Key::Media(MediaId(id))));
    assert_eq!(&shown[3..], std::slice::from_ref(&artist));
    for _ in 0..4 {
        tap(cx, "down");
    }
    let main = main_view(cx);
    assert!(cx.update(|_, cx| main.read(cx).library_panel.selection.contains(&artist)));
}

#[gpui::test]
fn a_sections_full_list_has_all_its_items(cx: &mut TestAppContext) {
    let home = Home { continue_watching: (1..=5).map(card).collect(), ..Home::default() };
    let cx = home_with(cx, home);
    let main = main_view(cx);
    let section = Route::HomeSection(HomeSection::ContinueWatching);
    main.update(cx, |main, cx| main.navigate(section, cx));
    assert_eq!(keys(cx).len(), 5);
}

#[test]
fn children_count_headings_and_the_away_note() {
    let shown = |section, cards, rows| Shown { section, cards, rows };
    let page = Page {
        cards: vec![card(1), card(2), card(3)],
        rows: Vec::new(),
        sections: vec![
            shown(HomeSection::ContinueWatching, 2, 0),
            shown(HomeSection::ListenAgain, 1, 0),
        ],
        history: true,
        unavailable: 4,
    };
    // Heading, E01, E02, "4 items unavailable", heading, E03.
    assert_eq!([0, 1, 2].map(|index| page.child_of(index)), [Some(1), Some(2), Some(5)]);
    assert_eq!(page.child_of(3), None);
    assert_eq!(
        page.away_note(HomeSection::ContinueWatching).as_deref(),
        Some("4 items unavailable")
    );
    assert_eq!(page.away_note(HomeSection::ListenAgain), None);
}

#[test]
fn a_folder_only_music_holds_opens_in_songs() {
    let mut settings = Settings::default();
    settings.local.media_folders = vec![PathBuf::from("/media")];
    settings.local.music_folders = vec![PathBuf::from("/music"), PathBuf::from("/media/Music")];
    let folders = ["/music/Juxt", "/media/Shows", "/media/Music/Live"].map(PathBuf::from);
    let targets: Vec<Route> = folder_rows(&folders, &settings)
        .iter()
        .filter_map(|row| match row.target() {
            Target::Go(route) => Some(route.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        targets,
        [
            Route::Music(MusicRoute::Songs(Some(folders[0].clone()))),
            Route::Videos(Some(folders[1].clone())),
            Route::Videos(Some(folders[2].clone())),
        ]
    );
}
