//! The command table: every command's label, where it applies, its default
//! keys and how it behaves when its key is held. Decision D3 settled the defaults;
//! see `docs/PRD.md`, Appendix B.

use std::time::Duration;

use gpui::Action;

use super::actions as act;
use Area::{App as AppArea, Global, Library, Player};

/// Where a command's shortcut works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    /// Every window, whatever has focus.
    Global,
    /// The main window, unless a modal is open.
    App,
    /// The player, unless a modal is open or a control needs the key.
    Player,
    /// The library (Stage 4), unless a modal is open.
    Library,
}

/// What happens while a shortcut's key is held and the OS repeats it (S0.5.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Repeat {
    /// Runs once per press; repeats are ignored until the key is released.
    Once,
    /// Runs again while held, at most once per interval.
    Throttled(Duration),
}

/// How often a held seek, frame-step or volume key acts: about 8 times a second,
/// slower than the OS repeat rate so the engine isn't flooded.
pub const HELD_KEY_INTERVAL: Duration = Duration::from_millis(120);

/// Default keys in GPUI's syntax. The first one is the primary shortcut: menus,
/// tooltips and onboarding show it; the others are aliases.
#[derive(Clone, Copy, Debug)]
pub enum Keys {
    /// The same keys on every OS (`secondary-` is Cmd on macOS, Ctrl elsewhere).
    All(&'static [&'static str]),
    /// Different keys on macOS and on Windows/Linux.
    PerOs {
        /// The keys on macOS.
        mac: &'static [&'static str],
        /// The keys on Windows and Linux.
        others: &'static [&'static str],
    },
}

impl Keys {
    /// The keys for `mac` (macOS) or Windows/Linux.
    pub fn for_os(self, mac: bool) -> &'static [&'static str] {
        match self {
            Keys::All(keys) => keys,
            Keys::PerOs { mac: keys, .. } if mac => keys,
            Keys::PerOs { others, .. } => others,
        }
    }
}

/// One user command.
pub struct Command {
    /// The name shown in menus and the shortcuts dialog.
    pub label: &'static str,
    /// Where its shortcut works.
    pub area: Area,
    /// Its default keys.
    pub keys: Keys,
    /// Its behavior while the key is held.
    pub repeat: Repeat,
    /// The action it dispatches.
    pub action: &'static dyn Action,
}

const NONE: Keys = Keys::All(&[]);

const fn cmd(label: &'static str, area: Area, keys: Keys, action: &'static dyn Action) -> Command {
    Command { label, area, keys, repeat: Repeat::Once, action }
}

/// A command whose held key acts repeatedly (seek, frame step, volume).
const fn held(label: &'static str, keys: Keys, action: &'static dyn Action) -> Command {
    Command { label, area: Player, keys, repeat: Repeat::Throttled(HELD_KEY_INTERVAL), action }
}

const fn keys(keys: &'static [&'static str]) -> Keys {
    Keys::All(keys)
}

/// Every command, in the order the shortcuts dialog lists them.
pub const COMMANDS: &[Command] = &[
    cmd("Open File(s)", AppArea, keys(&["secondary-o"]), &act::OpenFile),
    cmd("Open Folder", AppArea, keys(&["secondary-shift-o"]), &act::OpenFolder),
    cmd("Open Network Stream", AppArea, keys(&["secondary-n"]), &act::OpenNetworkStream),
    cmd("Open Disc", AppArea, keys(&["secondary-d"]), &act::OpenDisc),
    cmd("Open Capture Device", AppArea, keys(&["secondary-shift-c"]), &act::OpenCaptureDevice),
    cmd("Convert", AppArea, keys(&["secondary-r"]), &act::Convert),
    cmd("Search", Library, keys(&["secondary-f"]), &act::Search),
    cmd("Toggle fullscreen", Player, keys(&["f"]), &act::ToggleFullscreen),
    cmd("Exit fullscreen", Player, keys(&["escape"]), &act::ExitFullscreen),
    cmd("Show playback position and time", Player, keys(&["t"]), &act::ShowTime),
    cmd("Show interface controls in fullscreen", Player, keys(&["i"]), &act::ShowControls),
    cmd("Hide interface", Player, keys(&["secondary-\\"]), &act::HideInterface),
    cmd("Show / hide queue", Player, keys(&["q"]), &act::ToggleQueue),
    cmd("Show / hide file browser", Player, keys(&["l"]), &act::ToggleFileBrowser),
    cmd("Play / pause", Player, keys(&["space"]), &act::PlayPause),
    cmd("Stop playback", Player, keys(&["s"]), &act::Stop),
    // P/N are layout-safe; `<`/`>` (taught by onboarding's design) stay as aliases.
    cmd("Previous item", Player, keys(&["p", "<"]), &act::PreviousItem),
    cmd("Next item", Player, keys(&["n", ">"]), &act::NextItem),
    held("Seek backward", keys(&["left"]), &act::SeekBackward),
    held("Seek forward", keys(&["right"]), &act::SeekForward),
    held("Previous frame", keys(&[","]), &act::PreviousFrame),
    held("Next frame", keys(&["."]), &act::NextFrame),
    cmd("Next program / Service ID", Player, keys(&["x"]), &act::NextProgram),
    held("Increase volume", keys(&["up"]), &act::VolumeUp),
    held("Decrease volume", keys(&["down"]), &act::VolumeDown),
    cmd("Cycle audio track", Player, keys(&["b"]), &act::CycleAudioTrack),
    cmd("Cycle subtitle track", Player, keys(&["v"]), &act::CycleSubtitleTrack),
    cmd("Take a snapshot", Player, keys(&["shift-s"]), &act::Snapshot),
    cmd("Record", Player, keys(&["shift-r"]), &act::Record),
    cmd("Keyboard Shortcuts", AppArea, NONE, &act::ShowKeyboardShortcuts),
    cmd("Customize Player", AppArea, NONE, &act::CustomizePlayer),
    cmd(
        "Settings",
        AppArea,
        Keys::PerOs { mac: &["cmd-,"], others: &["ctrl-p"] },
        &act::OpenSettings,
    ),
    cmd("About VLC", AppArea, keys(&["shift-f1"]), &act::About),
    cmd("Run Setup Again", AppArea, NONE, &act::RunSetupAgain),
    cmd("Copy Debug Info", AppArea, NONE, &act::CopyDebugInfo),
    cmd(
        "Close Window",
        Global,
        // Windows and Linux close windows with Alt+F4 through the OS.
        Keys::PerOs { mac: &["cmd-w"], others: &[] },
        &act::CloseWindow,
    ),
    cmd("Quit", Global, keys(&["secondary-q"]), &act::Quit),
];

/// The command that dispatches `action`, if it is one of ours.
pub fn find(action: &dyn Action) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.action.partial_eq(action))
}
