//! A media card's file menu (S4.8.1; `library/menus/file_context_menu.png`): Play, Mark as
//! Watched (or Unwatched), Favorite, Add to Queue, Add to Playlist ▸, Show in Finder (File
//! Explorer, Files), Go to Source, Add Subtitle Tracks (the Add Subtitles dialog, S6.4), Add
//! Audio Tracks and Properties & Metadata.
//!
//! The design's Stream (S8.12) and Convert (S8.10) join when their stories ship; like the main menu, the menu leaves out what doesn't work yet. A file the
//! library hasn't identified yet can't be a favorite. A file that vanished while the menu was
//! open says "File not found" when shown or inspected.

use std::path::{Path, PathBuf};

use gpui::{App, AppContext as _, Context, WeakEntity, Window};

use super::MainWindow;
use super::item_menu::{FileTarget, on_all, on_one};
use super::selection::Key;
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::assets::icons;
use crate::db::activity::Favorite;
use crate::db::metadata::MediaKind;
use crate::history::History;
use crate::library::favorites;
use crate::main_window::add_subtitles::Target;
use crate::navigation::route::{MusicRoute, Route};
use crate::playlists;
use crate::settings;
use crate::ui::menu::MenuItem;
use crate::ui::toast::{Level, Toast};

/// What a file that is gone says.
const NOT_FOUND: &str = "File not found";

/// The file menu's items for `files`.
pub(super) fn items(
    main: &WeakEntity<MainWindow>,
    files: &[FileTarget],
    cx: &App,
) -> Vec<MenuItem> {
    let media: Vec<Media> = files.iter().map(|file| Media::File(file.path.clone())).collect();
    vec![
        on_all(main, files, "Play", icons::PLAY, |main, files, window, cx| match files {
            [file] => main.open_item(file.index, window, cx),
            files => main.play_cards(files_media(files), 0, window, cx),
        }),
        mark_watched(main, files),
        favorite(files, cx),
        on_all(main, files, "Add to Queue", icons::QUEUE, |main, files, window, cx| {
            let request = OpenRequest::placed(files_media(files), Placement::Enqueue);
            main.open(request, window, cx);
        }),
        add_to_playlist(main, &media, cx),
        on_one(main, files, show_in_folder_label(), icons::FOLDER, |_, file, _, cx| {
            MainWindow::reveal_file(file.path.clone(), cx);
        }),
        on_one(main, files, "Go to Source", icons::ARROW_RIGHT, |main, file, _, cx| {
            main.go_to_source(file, cx);
        }),
        on_one(main, files, "Add Subtitle Tracks", icons::SUBTITLES, |main, file, window, cx| {
            main.show_add_subtitles(Target::File(file.path.clone()), window, cx);
        }),
        on_one(main, files, "Add Audio Tracks", icons::AUDIO, |_, file, window, cx| {
            MainWindow::prompt_for_audio_tracks(Media::File(file.path.clone()), window, cx);
        }),
        on_one(main, files, "Properties & Metadata", icons::INFO, |main, file, window, cx| {
            main.show_file_properties(file.path.clone(), window, cx);
        }),
    ]
}

/// The files' media, in order.
fn files_media(files: &[FileTarget]) -> Vec<Media> {
    files.iter().map(|file| Media::File(file.path.clone())).collect()
}

/// Mark as Watched, or Mark as Unwatched once every one of `files` is watched.
fn mark_watched(main: &WeakEntity<MainWindow>, files: &[FileTarget]) -> MenuItem {
    let watched = !files.iter().all(|file| file.watched);
    let label = if watched { "Mark as Watched" } else { "Mark as Unwatched" };
    on_all(main, files, label, icons::CHECK_BOX_FULL, move |main, files, _, cx| {
        main.mark_cards_watched(files_media(files), watched, cx);
    })
}

/// Favorite, checked while every one of `files` is one; disabled while the library doesn't
/// know one of them yet.
fn favorite(files: &[FileTarget], cx: &App) -> MenuItem {
    let marked: Vec<Favorite> = files.iter().filter_map(|f| f.media.map(Favorite::Media)).collect();
    let unknown = marked.len() < files.len();
    let all = favorites::all(&marked, cx);
    MenuItem::check("Favorite", all, move |_, cx| favorites::mark(marked.clone(), !all, cx))
        .icon(icons::FAVORITE)
        .disabled(unknown)
        .why_disabled("Not in the library yet")
}

/// Add to Playlist ▸: the saved playlists, then New Playlist… holding `media`.
fn add_to_playlist(main: &WeakEntity<MainWindow>, media: &[Media], cx: &App) -> MenuItem {
    let mut items: Vec<MenuItem> = playlists::summaries(cx)
        .iter()
        .map(|summary| {
            let (main, summary, media) = (main.clone(), summary.clone(), media.to_vec());
            MenuItem::action(summary.title.clone(), move |window, cx| {
                main.update(cx, |_, cx| MainWindow::add_to_playlist(&summary, &media, window, cx))
                    .ok();
            })
        })
        .collect();
    if !items.is_empty() {
        items.push(MenuItem::Separator);
    }
    let (main, files) = (main.clone(), media.to_vec());
    items.push(MenuItem::action("New Playlist…", move |window, cx| {
        let files = files.clone();
        main.update(cx, |main, cx| main.show_new_playlist(files, window, cx)).ok();
    }));
    MenuItem::submenu("Add to Playlist", items).icon(icons::PLAYLIST)
}

/// What the OS's file manager is called: Show in Finder, File Explorer or Files.
pub(in crate::main_window) fn show_in_folder_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in File Explorer"
    } else {
        "Show in Files"
    }
}

/// The library route of `folder`, holding media of `kind`: inside one of the `media` folders
/// it is in Videos, inside one of the `music` folders in Songs, the one matching `kind` first.
/// `None` outside every one.
pub(super) fn source_route(
    folder: &Path,
    kind: MediaKind,
    (media, music): (&[PathBuf], &[PathBuf]),
) -> Option<Route> {
    let inside = |roots: &[PathBuf]| roots.iter().any(|root| folder.starts_with(root));
    let videos = inside(media).then(|| Route::Videos(Some(folder.to_path_buf())));
    let songs = inside(music).then(|| Route::Music(MusicRoute::Songs(Some(folder.to_path_buf()))));
    match kind {
        MediaKind::Audio => songs.or(videos),
        _ => videos.or(songs),
    }
}

impl MainWindow {
    /// Marks `media` watched or unwatched, then lists its folder again to show it.
    fn mark_cards_watched(&mut self, media: Vec<Media>, watched: bool, cx: &mut Context<Self>) {
        let Some(history) = History::get(cx) else { return };
        for media in media {
            history.set_watched(media, watched);
        }
        // The history thread runs jobs in order, so the folder reads after the change.
        self.reload_folder(cx);
    }

    /// Shows the file at `path` in the OS's file manager, or says it is gone. The disk is
    /// asked in the background: an offline network share can take a while to answer.
    fn reveal_file(path: PathBuf, cx: &mut Context<Self>) {
        let check = path.clone();
        let exists = cx.background_spawn(async move { check.exists() });
        cx.spawn(async move |this, cx| {
            let found = exists.await;
            this.update(cx, |this, cx| this.revealed(&path, found, cx)).ok();
        })
        .detach();
    }

    /// Shows the file at `path` in the OS's file manager if it was `found`, else says so.
    fn revealed(&mut self, path: &Path, found: bool, cx: &mut Context<Self>) {
        if found {
            cx.reveal_path(path);
        } else {
            self.show_toast(Toast::new(Level::Error, NOT_FOUND), cx);
        }
    }

    /// Goes to `file`'s folder in the library and selects it there; a file outside every media
    /// folder shows in the OS's file manager instead (S4.8 edge case).
    fn go_to_source(&mut self, file: &FileTarget, cx: &mut Context<Self>) {
        let Some(folder) = file.path.parent() else { return };
        let local = &settings::get(cx).local;
        let roots = (local.media_folders.as_slice(), local.music_folders.as_slice());
        let Some(route) = source_route(folder, file.kind, roots) else {
            Self::reveal_file(file.path.clone(), cx);
            return;
        };
        let key = file.media.map_or_else(|| Key::File(file.path.clone()), Key::Media);
        self.navigate(route, cx);
        self.library_panel.reveal = Some(key);
        // Already there, the listing won't be read again.
        self.select_revealed(cx);
    }

    /// Properties & Metadata of the file at `path`, or "File not found" when it is gone.
    fn show_file_properties(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if path.exists() {
            Self::show_properties_of(path, window, cx);
        } else {
            self.show_toast(Toast::new(Level::Error, NOT_FOUND), cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_source_is_in_the_section_of_its_kind() {
        let (media, music) = ([PathBuf::from("/m")], [PathBuf::from("/m/Music")]);
        let roots = (media.as_slice(), music.as_slice());
        let song = Path::new("/m/Music/Album");
        let songs = Route::Music(MusicRoute::Songs(Some(song.to_path_buf())));
        assert_eq!(source_route(song, MediaKind::Audio, roots), Some(songs));
        let videos = Route::Videos(Some(song.to_path_buf()));
        assert_eq!(source_route(song, MediaKind::Video, roots), Some(videos));
        assert_eq!(source_route(Path::new("/elsewhere"), MediaKind::Video, roots), None);
    }

    #[test]
    fn the_file_manager_has_its_own_name() {
        let label = show_in_folder_label();
        assert!(["Show in Finder", "Show in File Explorer", "Show in Files"].contains(&label));
    }
}
