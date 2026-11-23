//! The player's layout as data (S8.7): what the title bar and the control bar's rows show, in
//! what order. Customize Player edits a [`Layout`]; the bars are drawn from it.
//!
//! A layout is stored in `prefs.player_layout` by item ids (`stored.rs`), empty while it is the
//! designed one, so a later release's default reaches users who never customized it. Ids this
//! build doesn't know (a layout from a newer version) are skipped when drawing and kept, where
//! they were, when the layout is saved again.
//!
//! The time readouts and the seek bar are one unit, stored as `timeline`: they are three
//! [`Item`]s only so a narrow window can hide them one at a time (S2.2.5).

mod edit;
mod stored;

pub use self::edit::{Spot, Target, Unit};
use crate::settings::StoredLayout;

/// The most rows the control bar can have.
pub const MAX_ROWS: usize = 2;

/// One thing a bar can show. Each shows at most once in a layout; spacers and dividers are
/// [`Piece`]s instead, as there can be any number of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// The `…` main menu.
    Menu,
    /// Shows and hides the library sidebar (S4.5).
    Sidebar,
    /// The folder chip: the playing file's folder, opening it in the library (S2.1.3).
    Folder,
    /// What plays (S2.1.2). In the title bar it is centered on the window.
    Title,
    /// Restarts the item, or goes to the previous one.
    Previous,
    /// Jumps back 15 s.
    Back15,
    /// Plays or pauses.
    PlayPause,
    /// Jumps forward 15 s.
    Forward15,
    /// Goes to the next item.
    Next,
    /// The playback position (`00:02:10`), the timeline's start.
    CurrentTime,
    /// The seek bar; it takes the room the other items leave.
    SeekBar,
    /// The media's length (`00:25:40`), the timeline's end.
    Duration,
    /// The speaker (mute) and the volume wedge.
    Volume,
    /// The audio tracks.
    Audio,
    /// The subtitle tracks.
    Subtitles,
    /// The playback settings panel.
    Settings,
    /// The queue.
    List,
    /// Jumps back 10 s.
    Back10,
    /// Jumps forward 10 s.
    Forward10,
    /// The playback speed, in the settings panel.
    Speed,
    /// Stops playback.
    Stop,
    /// Moves Repeat to its next mode.
    Repeat,
    /// Switches Shuffle on or off.
    Shuffle,
    /// Starts or stops recording (S8.6).
    Record,
    /// Saves the picture on screen (S8.4).
    Snapshot,
    /// Pops the player out into its own small window (S8.5).
    PictureInPicture,
    /// Quits the app.
    Quit,
    /// Opens the Settings window.
    VlcSettings,
    /// Opens the library at Home.
    GoHome,
    /// The playing file's name.
    FileName,
    /// The name of the playing file's folder.
    FolderName,
    /// Shows the playing media's tags and details (S4.3.5).
    Properties,
    /// Plays the media again from its start.
    Replay,
    /// The queue panel's Chapters tab (S8.2).
    Chapters,
    /// Bookmarks where playback is (S8.3).
    Bookmark,
    /// The queue panel's Bookmarks tab (S8.3).
    Bookmarks,
    /// Opens the library's saved playlists.
    Playlists,
}

/// One entry in a bar's layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Piece {
    /// A control.
    Item(Item),
    /// Space that grows to take the room the bar has left, shared with the seek bar.
    Spacer,
    /// A thin vertical line between two groups.
    Divider,
    /// Extra space between two of the designed groups, in pixels, on top of the usual gap
    /// between items. It isn't moved on its own: it stays where it is as items move around it.
    Gap(f32),
}

/// Where a piece sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Zone {
    /// The title bar, after the logo.
    TitleBar,
    /// A control bar row, counted from the top.
    Row(usize),
}

/// An id this build doesn't know, kept to be saved again where it was.
#[derive(Clone, Debug, PartialEq)]
struct Unknown {
    zone: Zone,
    /// Its place among the zone's stored ids, so it goes back near the same neighbours.
    index: usize,
    id: String,
}

/// What the title bar and the control bar show.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The title bar's pieces after the logo, left to right. The title, when here, is drawn
    /// centered on the window rather than in line.
    pub title_bar: Vec<Piece>,
    /// The control bar's rows, top to bottom: one or [`MAX_ROWS`].
    pub rows: Vec<Vec<Piece>>,
    unknown: Vec<Unknown>,
}

impl Layout {
    /// The designed player (`player_01.png`, `controls-default.png`).
    pub fn designed() -> Self {
        Self { title_bar: designed_title_bar(), rows: vec![designed_row()], unknown: Vec::new() }
    }

    /// The layout `stored` describes, or the designed one when it's empty.
    pub fn from_stored(stored: &StoredLayout) -> Self {
        stored::parse(stored)
    }

    /// This layout as stored: empty when it is the designed one.
    pub fn to_stored(&self) -> StoredLayout {
        stored::write(self)
    }
}

fn designed_title_bar() -> Vec<Piece> {
    [Item::Menu, Item::Sidebar, Item::Folder, Item::Title].map(Piece::Item).to_vec()
}

/// The designed control bar (`controls-default.png`): transport, then time and seeking, the
/// volume, and the panel buttons.
fn designed_row() -> Vec<Piece> {
    use Item::{
        Audio, CurrentTime, Duration, List, Next, PlayPause, Previous, SeekBar, Settings,
        Subtitles, Volume,
    };
    vec![
        Piece::Item(Previous),
        Piece::Item(PlayPause),
        Piece::Item(Next),
        Piece::Gap(16.),
        Piece::Item(CurrentTime),
        Piece::Item(SeekBar),
        Piece::Item(Duration),
        Piece::Gap(12.),
        Piece::Item(Volume),
        Piece::Gap(8.),
        Piece::Item(Audio),
        Piece::Item(Subtitles),
        Piece::Item(Settings),
        Piece::Item(List),
    ]
}

/// Music mode's bar (S8.1.2, `controls-music.png`): the transport gains Back 15 s and Forward
/// 15 s, and Subtitles goes. Settings stays, as `controls-music.png` has it: its panel holds
/// the speed and the Audio Visualizer, which music wants as much as video does. Customize
/// Player edits the video layout; music mode keeps this one.
pub fn music_row() -> Vec<Piece> {
    use Item::{
        Audio, Back15, CurrentTime, Duration, Forward15, List, Next, PlayPause, Previous, SeekBar,
        Settings, Volume,
    };
    vec![
        Piece::Item(Previous),
        Piece::Item(Back15),
        Piece::Item(PlayPause),
        Piece::Item(Forward15),
        Piece::Item(Next),
        Piece::Gap(16.),
        Piece::Item(CurrentTime),
        Piece::Item(SeekBar),
        Piece::Item(Duration),
        Piece::Gap(12.),
        Piece::Item(Volume),
        Piece::Gap(8.),
        Piece::Item(Audio),
        Piece::Item(Settings),
        Piece::Item(List),
    ]
}
