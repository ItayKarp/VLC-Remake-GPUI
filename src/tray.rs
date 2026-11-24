//! The system tray icon (S8.16.1): VLC's cone in the menu bar on macOS, the notification area on
//! Windows and a StatusNotifierItem on Linux desktops that show one, with a menu of Play/Pause,
//! Previous, Next, Show VLC and Quit VLC.
//!
//! GPUI has no tray API, so each OS's own is used: an `NSStatusItem` on macOS, `Shell_NotifyIcon`
//! on Windows, and the StatusNotifierItem and dbusmenu D-Bus interfaces on Linux. The icon only
//! sends [`TrayCommand`]s; the app runs them (`app/system_tray.rs`).
//!
//! GNOME shows no tray without the AppIndicator extension: there, and on other desktops without a
//! StatusNotifierWatcher, [`unavailable_hint`] says so under the setting and no icon is made.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use futures::channel::mpsc::UnboundedSender;
use gpui::App;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// The icon, a PNG the platforms scale to their tray's size.
#[cfg_attr(
    target_os = "windows",
    allow(dead_code, reason = "Windows uses the icon resource embedded in the executable")
)]
const ICON_PNG: &[u8] = include_bytes!("../assets/app-icon/icon-32.png");

/// What the tray menu (or clicking the icon) asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    /// Plays or pauses what is loaded.
    PlayPause,
    /// Goes to the previous item.
    Previous,
    /// Goes to the next item.
    Next,
    /// Brings the main window forward, showing it if it was hidden.
    ShowVlc,
    /// Quits, as the app menu's Quit does.
    Quit,
}

impl TrayCommand {
    /// The menu, in groups a separator divides.
    pub const MENU: &[&[Self]] =
        &[&[Self::PlayPause, Self::Previous, Self::Next], &[Self::ShowVlc, Self::Quit]];

    /// Every command, in menu order; a command's index here is its id in the OS's menu.
    const ALL: [Self; 5] = [Self::PlayPause, Self::Previous, Self::Next, Self::ShowVlc, Self::Quit];

    /// The menu item's text, in the interface language.
    pub fn label(self) -> &'static str {
        match self {
            Self::PlayPause => tr!("Play/Pause"),
            Self::Previous => tr!("Previous"),
            Self::Next => tr!("Next"),
            Self::ShowVlc => tr!("Show VLC"),
            Self::Quit => tr!("Quit VLC"),
        }
    }

    /// The id the OS's menu item carries, never 0 (Windows' "nothing chosen").
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
        allow(dead_code, reason = "no tray here")
    )]
    fn id(self) -> i32 {
        let index = Self::ALL.iter().position(|command| *command == self);
        index.map_or(0, |index| index as i32 + 1)
    }

    /// The command whose menu item carries `id`.
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "macos", target_os = "windows")),
        allow(dead_code, reason = "no tray here")
    )]
    fn from_id(id: i64) -> Option<Self> {
        let index = usize::try_from(id.checked_sub(1)?).ok()?;
        Self::ALL.get(index).copied()
    }
}

/// Where the icon sends what was chosen.
pub type CommandSender = UnboundedSender<TrayCommand>;

/// The icon while it shows; dropping it takes it away.
pub struct TrayIcon {
    _icon: platform::Icon,
}

impl TrayIcon {
    /// Shows the icon, sending what is chosen in its menu to `sender`. Call it on the main
    /// thread.
    pub fn show(sender: CommandSender) -> anyhow::Result<Self> {
        Ok(Self { _icon: platform::Icon::show(sender)? })
    }
}

/// Under "Show VLC in the system tray": why there is no tray to show it in, if there isn't.
pub fn unavailable_hint(_: &App) -> Option<String> {
    (!platform::is_available()).then(|| {
        tr!("This desktop has no system tray. On GNOME, the AppIndicator extension adds one.")
            .to_owned()
    })
}

/// Whether the desktop has a tray to show an icon in.
pub fn is_available() -> bool {
    platform::is_available()
}

/// Where no tray is known: no icon.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod platform {
    use super::CommandSender;

    pub struct Icon;

    impl Icon {
        pub fn show(_: CommandSender) -> anyhow::Result<Self> {
            anyhow::bail!("no system tray on this OS")
        }
    }

    pub fn is_available() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_name_their_commands() {
        for command in TrayCommand::ALL {
            assert_ne!(command.id(), 0);
            assert_eq!(TrayCommand::from_id(i64::from(command.id())), Some(command));
        }
        assert_eq!(TrayCommand::from_id(0), None);
        assert_eq!(TrayCommand::from_id(6), None);
        assert_eq!(TrayCommand::from_id(-1), None);
    }

    #[test]
    fn the_menu_holds_every_command_once() {
        let listed: Vec<_> = TrayCommand::MENU.iter().flat_map(|group| group.iter()).collect();
        assert_eq!(listed.len(), TrayCommand::ALL.len());
        assert!(TrayCommand::ALL.iter().all(|command| listed.contains(&command)));
    }
}
