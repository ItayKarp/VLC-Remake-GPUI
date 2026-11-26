//! Each unit's name and icon in the palette and while dragged (`player_customize_01.png`).

use crate::assets::icons;
use crate::player_layout::{Item, Unit};

/// `unit`'s name in the palette, in the interface language.
pub(in crate::main_window) fn label(unit: Unit) -> &'static str {
    crate::i18n::text(english(unit))
}

/// `unit`'s name in English: the key [`label`] translates, and the name test selectors use.
#[allow(clippy::too_many_lines, reason = "a flat table, a line per item")]
pub(super) fn english(unit: Unit) -> &'static str {
    let item = match unit {
        Unit::Spacer => return tr_noop!("Spacer"),
        Unit::Divider => return tr_noop!("Divider"),
        Unit::Item(item) => item,
    };
    match item {
        Item::Menu => tr_noop!("Menu"),
        Item::Sidebar => tr_noop!("File Browser"),
        Item::Folder => tr_noop!("Folder"),
        Item::Title => tr_noop!("Title"),
        Item::Previous => tr_noop!("Previous"),
        Item::Back15 => tr_noop!("Backward 15s"),
        Item::PlayPause => tr_noop!("Play/Pause"),
        Item::Forward15 => tr_noop!("Forward 15s"),
        Item::Next => tr_noop!("Next"),
        Item::CurrentTime | Item::SeekBar | Item::Duration => tr_noop!("Timeline"),
        Item::Volume => tr_noop!("Volume"),
        Item::Audio => tr_noop!("Audio"),
        Item::Subtitles => tr_noop!("Subtitles"),
        Item::Settings => tr_noop!("Playback Settings"),
        Item::List => tr_noop!("Queue"),
        Item::Back10 => tr_noop!("Backward 10s"),
        Item::Forward10 => tr_noop!("Forward 10s"),
        Item::Speed => tr_noop!("Playback Speed"),
        Item::Stop => tr_noop!("Stop"),
        Item::Repeat => tr_noop!("Repeat"),
        Item::Shuffle => tr_noop!("Shuffle"),
        Item::Record => tr_noop!("Record"),
        Item::Snapshot => tr_noop!("Snapshot"),
        Item::PictureInPicture => tr_noop!("Picture in Picture"),
        Item::Quit => tr_noop!("Quit VLC"),
        Item::VlcSettings => tr_noop!("VLC Settings"),
        Item::GoHome => tr_noop!("Go Home"),
        Item::FileName => tr_noop!("File Name"),
        Item::FolderName => tr_noop!("Folder Name"),
        Item::Properties => tr_noop!("Properties"),
        Item::Replay => tr_noop!("Replay"),
        Item::Chapters => tr_noop!("Chapters"),
        Item::Bookmark => tr_noop!("Bookmark"),
        Item::Bookmarks => tr_noop!("Bookmarks"),
        Item::Playlists => tr_noop!("Playlists"),
        Item::Favorite => tr_noop!("Favorite"),
        Item::Hdr => tr_noop!("HDR"),
        Item::Zoom => tr_noop!("Zoom"),
        Item::RightSidebar => tr_noop!("Right Sidebar"),
        Item::Thumbnail => tr_noop!("Thumbnail"),
        Item::ArtistName => tr_noop!("Artist Name"),
        Item::PlaylistName => tr_noop!("Playlist Name"),
        Item::Minimize => tr_noop!("Minimize"),
        Item::Maximize => tr_noop!("Maximize"),
        Item::Fullscreen => tr_noop!("Fullscreen"),
        Item::Close => tr_noop!("Close"),
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
