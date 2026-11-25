//! What VLC's StatusNotifierItem and its dbusmenu say: the item's properties, the menu's
//! entries and their ids, which click runs which command, and the icon in the protocol's pixel
//! format. It knows nothing of D-Bus, so it is built and tested on every OS; `linux.rs` only
//! serves it over the session bus.

use super::TrayCommand;

/// The item's category: an application's status, not a hardware or system one.
pub const CATEGORY: &str = "ApplicationStatus";
/// The item's id, unique to the application.
pub const ID: &str = "vlc";
/// The item's title, and its tooltip's.
pub const TITLE: &str = "VLC";
/// The item's status: always shown, never hidden as passive or flashing for attention.
pub const STATUS: &str = "Active";
/// The menu's root entry's id.
pub const ROOT_ID: i32 = 0;
/// Separators' ids are this plus the index of the group they open, past every command's id.
const SEPARATOR_IDS: i32 = 100;
/// The dbusmenu event a click sends.
const CLICKED: &str = "clicked";

/// An icon image: width, height and ARGB32 pixels in network byte order.
pub type Pixmap = (i32, i32, Vec<u8>);

/// An entry of the icon's menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// The invisible root, whose children are the menu.
    Root,
    /// A line between two groups of commands.
    Separator,
    /// A command's item.
    Command(TrayCommand),
}

impl Entry {
    /// The entry carrying `id`; `None` for an id the menu doesn't have.
    pub fn from_id(id: i32) -> Option<Self> {
        if id == ROOT_ID {
            return Some(Self::Root);
        }
        if separator_ids().any(|separator| separator == id) {
            return Some(Self::Separator);
        }
        TrayCommand::from_id(i64::from(id)).map(Self::Command)
    }

    /// The entry's dbusmenu property: its name and value. Every entry has exactly one.
    pub fn property(self) -> (&'static str, &'static str) {
        match self {
            Self::Root => ("children-display", "submenu"),
            Self::Separator => ("type", "separator"),
            Self::Command(command) => ("label", command.label()),
        }
    }
}

/// The ids of the root's children, in menu order: the commands, with a separator between
/// their groups.
pub fn menu_ids() -> Vec<i32> {
    let mut ids = Vec::new();
    for (index, group) in TrayCommand::MENU.iter().enumerate() {
        if index > 0 {
            ids.push(separator_id(index));
        }
        ids.extend(group.iter().map(|command| command.id()));
    }
    ids
}

/// The command a dbusmenu `event` on entry `id` runs, if it runs one.
pub fn chosen(id: i32, event: &str) -> Option<TrayCommand> {
    match Entry::from_id(id)? {
        Entry::Command(command) if event == CLICKED => Some(command),
        _ => None,
    }
}

/// `png` as the protocol's pixmap.
pub fn pixmap(png: &[u8]) -> anyhow::Result<Pixmap> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.to_rgba8();
    let (width, height) = image.dimensions();
    let argb = image.pixels().flat_map(|pixel| {
        let [red, green, blue, alpha] = pixel.0;
        [alpha, red, green, blue]
    });
    Ok((i32::try_from(width)?, i32::try_from(height)?, argb.collect()))
}

fn separator_id(group: usize) -> i32 {
    SEPARATOR_IDS + i32::try_from(group).expect("the menu has a handful of groups")
}

fn separator_ids() -> impl Iterator<Item = i32> {
    (1..TrayCommand::MENU.len()).map(separator_id)
}

#[cfg(test)]
mod tests;
