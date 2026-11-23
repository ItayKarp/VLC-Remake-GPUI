//! How each layout [`Item`] takes part in fitting a bar to a narrow window (S2.2.5): how wide it
//! is at least and how readily it gives way. `fit.rs` decides what stays. What the bar shows
//! comes from the player layout (`player_layout`, S8.7).

use crate::player_layout::Item;

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
/// The time readouts' width: `-00:00:00` at the bar's text size, inside the hover pill.
pub const TIME_WIDTH: f32 = 72.;
/// The narrowest the seek bar gets before it gives way.
pub const SEEK_MIN: f32 = 120.;
/// The icons' size inside the bar's buttons.
pub const ICON: f32 = 20.;
/// The volume wedge's width after the speaker, with room for its focus ring (S2.5.1).
pub const LEVEL_WIDTH: f32 = 93.;
/// The space between the speaker and the volume wedge.
pub const LEVEL_GAP: f32 = 6.;
/// The widest a name (the title, the file's or the folder's) gets in the bar; past it, it's cut
/// with an ellipsis.
pub const NAME_WIDTH: f32 = 160.;
/// The folder chip's width in the bar, its name cut to fit.
pub const CHIP_WIDTH: f32 = 120.;
/// The Thumbnail item's width: a 16:9 tile as tall as a button.
pub const THUMBNAIL_WIDTH: f32 = 50.;
/// A divider's line, with the room around it.
pub const DIVIDER_WIDTH: f32 = 1.;

/// How `item` takes part in fitting the bar.
pub fn fit(item: Item) -> Fit {
    let (width, priority, dropped) = match item {
        Item::PlayPause => (PILL_WIDTH, u8::MAX, Dropped::Never),
        // The main menu has no other way in off macOS.
        Item::Menu => (BUTTON, u8::MAX, Dropped::Never),
        Item::SeekBar => (SEEK_MIN, 90, Dropped::Hidden),
        Item::CurrentTime => (TIME_WIDTH, 80, Dropped::Hidden),
        Item::Previous | Item::Next => (BUTTON, 70, Dropped::Overflow),
        Item::Back15 | Item::Forward15 | Item::Back10 | Item::Forward10 => {
            (BUTTON, 65, Dropped::Overflow)
        }
        Item::Duration => (TIME_WIDTH, 60, Dropped::Hidden),
        Item::Volume => (BUTTON + LEVEL_GAP + LEVEL_WIDTH, 50, Dropped::Overflow),
        Item::Audio | Item::Subtitles => (BUTTON, 40, Dropped::Overflow),
        Item::List => (BUTTON, 30, Dropped::Overflow),
        Item::Settings => (BUTTON, 20, Dropped::Overflow),
        Item::Title | Item::FileName | Item::FolderName | Item::ArtistName | Item::PlaylistName => {
            (NAME_WIDTH, 5, Dropped::Hidden)
        }
        Item::Thumbnail => (THUMBNAIL_WIDTH, 5, Dropped::Hidden),
        Item::Folder => (CHIP_WIDTH, 5, Dropped::Hidden),
        _ => (BUTTON, 10, Dropped::Overflow),
    };
    Fit { width, priority, dropped }
}
