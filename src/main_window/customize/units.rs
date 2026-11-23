//! Each unit's name and icon in the palette and while dragged (`player_customize_01.png`).

use crate::assets::icons;
use crate::player_layout::{Item, Unit};

/// `unit`'s name in the palette.
#[allow(clippy::too_many_lines, reason = "a flat table, a line per item")]
pub(in crate::main_window) fn label(unit: Unit) -> &'static str {
    let item = match unit {
        Unit::Spacer => return "Spacer",
        Unit::Divider => return "Divider",
        Unit::Item(item) => item,
    };
    match item {
        Item::Menu => "Menu",
        Item::Sidebar => "File Browser",
        Item::Folder => "Folder",
        Item::Title => "Title",
        Item::Previous => "Previous",
        Item::Back15 => "Backward 15s",
        Item::PlayPause => "Play/Pause",
        Item::Forward15 => "Forward 15s",
        Item::Next => "Next",
        Item::CurrentTime | Item::SeekBar | Item::Duration => "Timeline",
        Item::Volume => "Volume",
        Item::Audio => "Audio",
        Item::Subtitles => "Subtitles",
        Item::Settings => "Playback Settings",
        Item::List => "Queue",
        Item::Back10 => "Backward 10s",
        Item::Forward10 => "Forward 10s",
        Item::Speed => "Playback Speed",
        Item::Stop => "Stop",
        Item::Repeat => "Repeat",
        Item::Shuffle => "Shuffle",
        Item::Record => "Record",
        Item::Snapshot => "Snapshot",
        Item::PictureInPicture => "Picture in Picture",
        Item::Quit => "Quit VLC",
        Item::VlcSettings => "VLC Settings",
        Item::GoHome => "Go Home",
        Item::FileName => "File Name",
        Item::FolderName => "Folder Name",
        Item::Properties => "Properties",
        Item::Replay => "Replay",
        Item::Chapters => "Chapters",
        Item::Bookmark => "Bookmark",
        Item::Bookmarks => "Bookmarks",
        Item::Playlists => "Playlists",
        Item::Favorite => "Favorite",
        Item::Hdr => "HDR",
        Item::Zoom => "Zoom",
        Item::RightSidebar => "Right Sidebar",
        Item::Thumbnail => "Thumbnail",
        Item::ArtistName => "Artist Name",
        Item::PlaylistName => "Playlist Name",
        Item::Minimize => "Minimize",
        Item::Maximize => "Maximize",
        Item::Fullscreen => "Fullscreen",
        Item::Close => "Close",
    }
}

/// `unit`'s icon in the palette; none for Spacer and Divider, which are drawn.
#[allow(clippy::too_many_lines, reason = "a flat table, a line per item")]
pub(super) fn icon(unit: Unit) -> Option<&'static str> {
    let Unit::Item(item) = unit else { return None };
    Some(match item {
        Item::Menu => icons::MORE,
        Item::Sidebar => icons::SIDEBAR,
        Item::Folder => icons::FOLDER,
        Item::Previous => icons::PREVIOUS,
        Item::Back15 => icons::BACK_15,
        Item::PlayPause => icons::PLAY,
        Item::Forward15 => icons::FORWARD_15,
        Item::Next => icons::NEXT,
        Item::CurrentTime | Item::SeekBar | Item::Duration => icons::PLAYBACK,
        Item::Volume => icons::VOLUME,
        Item::Audio => icons::AUDIO,
        Item::Subtitles => icons::SUBTITLES,
        Item::Settings | Item::VlcSettings => icons::SETTINGS,
        Item::List => icons::LIST,
        Item::Back10 => icons::BACK_10,
        Item::Forward10 => icons::FORWARD_10,
        Item::Speed => icons::SPEED,
        Item::Stop => icons::STOP,
        Item::Repeat => icons::REPEAT,
        Item::Shuffle => icons::SHUFFLE,
        Item::Record => icons::RECORD,
        Item::Snapshot => icons::SNAPSHOT,
        Item::PictureInPicture => icons::PIP,
        Item::Quit => icons::QUIT,
        Item::GoHome => icons::HOME,
        Item::Title
        | Item::FileName
        | Item::FolderName
        | Item::ArtistName
        | Item::PlaylistName
        | Item::Properties => icons::INFO,
        Item::Favorite => icons::FAVORITE,
        Item::Hdr => icons::HDR,
        Item::Zoom => icons::ZOOM,
        Item::RightSidebar => icons::SIDEBAR_RIGHT,
        Item::Thumbnail => icons::THUMBNAIL,
        Item::Replay => icons::REPLAY,
        Item::Chapters => icons::CHAPTERS,
        Item::Bookmark => icons::BOOKMARK,
        Item::Bookmarks => icons::BOOKMARKS,
        Item::Playlists => icons::PLAYLIST,
        Item::Minimize => icons::MINIMIZE,
        Item::Maximize => icons::MAXIMIZE,
        Item::Fullscreen => icons::FULLSCREEN,
        Item::Close => icons::CLOSE,
    })
}
