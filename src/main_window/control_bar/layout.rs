//! The control bar as data (S2.2.1): which items it shows, in what order, and the gaps that
//! split them into groups. The bar is drawn from a [`Slot`] list, so Customize Player (S8.7)
//! can rearrange it by editing the list.
//!
//! Each [`Item`] also says how wide it is at least and how readily it gives way on a narrow
//! window (S2.2.5); `fit.rs` decides what stays.

/// One control the bar can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    /// Restarts the item, or goes to the previous one.
    Previous,
    /// Plays or pauses.
    PlayPause,
    /// Goes to the next item.
    Next,
    /// The playback position (`00:02:10`).
    CurrentTime,
    /// The seek bar; it takes the room the other items leave.
    SeekBar,
    /// The media's length (`00:25:40`).
    Duration,
    /// The speaker (mute) and the volume level.
    Volume,
    /// The audio tracks.
    Audio,
    /// The subtitle tracks.
    Subtitles,
    /// The playback settings panel.
    Settings,
    /// The queue.
    List,
}

/// One entry in the bar's layout.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    /// A control.
    Item(Item),
    /// Extra space between two groups, in pixels, on top of the usual gap between items.
    Gap(f32),
}

/// What happens to an item that doesn't fit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dropped {
    /// It moves into the overflow `…` menu.
    Overflow,
    /// It is hidden: a menu entry would mean nothing (a time readout).
    Hidden,
    /// It always stays.
    Never,
}

/// How an item takes part in fitting the bar.
#[derive(Clone, Copy, Debug)]
pub struct Fit {
    /// Its width, or its narrowest for the seek bar, in pixels.
    pub width: f32,
    /// The lower, the sooner it gives way. Equal ones give way right to left.
    pub priority: u8,
    /// Where it goes when it gives way.
    pub dropped: Dropped,
}

/// An icon button's side in the bar (`controls-default.png`).
pub const BUTTON: f32 = 28.;
/// The Play/Pause pill's size.
pub const PILL_WIDTH: f32 = 54.;
/// The Play/Pause pill's height.
pub const PILL_HEIGHT: f32 = 26.;
/// The time readouts' width: `00:00:00` at the bar's text size.
pub const TIME_WIDTH: f32 = 62.;
/// The narrowest the seek bar gets before it gives way.
pub const SEEK_MIN: f32 = 120.;
/// The volume level's width, after the speaker.
pub const LEVEL_WIDTH: f32 = 90.;
/// The space between the speaker and the volume level.
pub const LEVEL_GAP: f32 = 6.;

impl Item {
    /// How this item takes part in fitting the bar.
    pub fn fit(self) -> Fit {
        let (width, priority, dropped) = match self {
            Item::PlayPause => (PILL_WIDTH, u8::MAX, Dropped::Never),
            Item::SeekBar => (SEEK_MIN, 90, Dropped::Hidden),
            Item::CurrentTime => (TIME_WIDTH, 80, Dropped::Hidden),
            Item::Previous | Item::Next => (BUTTON, 70, Dropped::Overflow),
            Item::Duration => (TIME_WIDTH, 60, Dropped::Hidden),
            Item::Volume => (BUTTON + LEVEL_GAP + LEVEL_WIDTH, 50, Dropped::Overflow),
            Item::Audio | Item::Subtitles => (BUTTON, 40, Dropped::Overflow),
            Item::List => (BUTTON, 30, Dropped::Overflow),
            Item::Settings => (BUTTON, 20, Dropped::Overflow),
        };
        Fit { width, priority, dropped }
    }
}

/// The designed bar (`controls-default.png`): transport, then time and seeking, the volume,
/// and the panel buttons.
pub fn default_layout() -> Vec<Slot> {
    use Item::{
        Audio, CurrentTime, Duration, List, Next, PlayPause, Previous, SeekBar, Settings,
        Subtitles, Volume,
    };
    vec![
        Slot::Item(Previous),
        Slot::Item(PlayPause),
        Slot::Item(Next),
        Slot::Gap(16.),
        Slot::Item(CurrentTime),
        Slot::Item(SeekBar),
        Slot::Item(Duration),
        Slot::Gap(12.),
        Slot::Item(Volume),
        Slot::Gap(8.),
        Slot::Item(Audio),
        Slot::Item(Subtitles),
        Slot::Item(Settings),
        Slot::Item(List),
    ]
}
