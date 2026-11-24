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

impl Area {
    /// Whether a key could reach commands of both areas, so they can't share it. The player
    /// and the library can: the library's key wins while it has focus.
    pub fn overlaps(self, other: Area) -> bool {
        self == other || [self, other].iter().any(|area| matches!(area, Global | AppArea))
    }
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
    /// The English name, which identifies the command (menu bar items, held keys); menus and
    /// the shortcuts dialog show [`Command::title`].
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

impl Command {
    /// The name shown in menus and the shortcuts dialog, in the interface language.
    pub fn title(&self) -> &'static str {
        crate::i18n::text(self.label)
    }
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
    cmd(tr_noop!("Open File(s)"), AppArea, keys(&["secondary-o"]), &act::OpenFile),
    cmd(tr_noop!("Open Folder"), AppArea, keys(&["secondary-shift-o"]), &act::OpenFolder),
    // A text input's own paste binding is deeper in the key context, so it wins there.
    cmd(tr_noop!("Open from Clipboard"), AppArea, keys(&["secondary-v"]), &act::OpenFromClipboard),
    cmd(tr_noop!("Open Network Stream"), AppArea, keys(&["secondary-n"]), &act::OpenNetworkStream),
    cmd(tr_noop!("Open Disc"), AppArea, keys(&["secondary-d"]), &act::OpenDisc),
    cmd(
        tr_noop!("Open Capture Device"),
        AppArea,
        keys(&["secondary-shift-c"]),
        &act::OpenCaptureDevice,
    ),
    cmd(tr_noop!("Convert"), AppArea, keys(&["secondary-r"]), &act::Convert),
    cmd(tr_noop!("Stream"), AppArea, keys(&["secondary-s"]), &act::Stream),
    cmd(tr_noop!("Search"), Library, keys(&["secondary-f"]), &act::Search),
    // Alt+Left and Alt+Right seek in the player; the library's are deeper in the key context, so
    // they win while it has focus.
    cmd(
        tr_noop!("Back"),
        Library,
        Keys::PerOs { mac: &["cmd-[", "alt-left"], others: &["alt-left"] },
        &act::GoBack,
    ),
    cmd(
        tr_noop!("Forward"),
        Library,
        Keys::PerOs { mac: &["cmd-]", "alt-right"], others: &["alt-right"] },
        &act::GoForward,
    ),
    cmd(tr_noop!("Toggle fullscreen"), Player, keys(&["f"]), &act::ToggleFullscreen),
    cmd(tr_noop!("Exit fullscreen"), Player, keys(&["escape"]), &act::ExitFullscreen),
    cmd(tr_noop!("Show playback position and time"), Player, keys(&["t"]), &act::ShowTime),
    cmd(
        tr_noop!("Show interface controls in fullscreen"),
        Player,
        keys(&["i"]),
        &act::ShowControls,
    ),
    cmd(tr_noop!("Hide interface"), Player, keys(&["secondary-\\"]), &act::HideInterface),
    cmd(tr_noop!("Show / hide queue"), Player, keys(&["q"]), &act::ToggleQueue),
    cmd(tr_noop!("Show / hide file browser"), Player, keys(&["l"]), &act::ToggleFileBrowser),
    cmd(tr_noop!("Play / pause"), Player, keys(&["space"]), &act::PlayPause),
    cmd(tr_noop!("Stop playback"), Player, keys(&["s"]), &act::Stop),
    // P/N are layout-safe; `<`/`>` (taught by onboarding's design) stay as aliases.
    cmd(tr_noop!("Previous item"), Player, keys(&["p", "<"]), &act::PreviousItem),
    cmd(tr_noop!("Next item"), Player, keys(&["n", ">"]), &act::NextItem),
    // VLC's keys: Shift with the queue's P and N.
    cmd(tr_noop!("Previous chapter"), Player, keys(&["shift-p"]), &act::PreviousChapter),
    cmd(tr_noop!("Next chapter"), Player, keys(&["shift-n"]), &act::NextChapter),
    // VLC has no key for it; B is the audio track, so the platform's modifier goes with it.
    cmd(tr_noop!("Add bookmark"), Player, keys(&["secondary-b"]), &act::AddBookmark),
    // No default keys: VLC's L (loop) is the file browser here (D3), and they live in menus.
    cmd(tr_noop!("Repeat (Off, All, One)"), Player, NONE, &act::CycleRepeat),
    cmd(tr_noop!("Shuffle"), Player, NONE, &act::ToggleShuffle),
    // VLC's key; L alone is the file browser here (D3).
    cmd(tr_noop!("A-B loop"), Player, keys(&["shift-l"]), &act::LoopAToB),
    // The skip amounts (S2.3.4): Shift is finer, Alt (Option) and Ctrl (Cmd) coarser.
    held(tr_noop!("Seek backward"), keys(&["left"]), &act::SeekBackward),
    held(tr_noop!("Seek forward"), keys(&["right"]), &act::SeekForward),
    held(tr_noop!("Very short seek backward"), keys(&["shift-left"]), &act::SeekBackwardVeryShort),
    held(tr_noop!("Very short seek forward"), keys(&["shift-right"]), &act::SeekForwardVeryShort),
    held(tr_noop!("Medium seek backward"), keys(&["alt-left"]), &act::SeekBackwardMedium),
    held(tr_noop!("Medium seek forward"), keys(&["alt-right"]), &act::SeekForwardMedium),
    held(tr_noop!("Long seek backward"), keys(&["secondary-left"]), &act::SeekBackwardLong),
    held(tr_noop!("Long seek forward"), keys(&["secondary-right"]), &act::SeekForwardLong),
    cmd(tr_noop!("Go to the start"), Player, keys(&["home"]), &act::SeekToStart),
    cmd(tr_noop!("Go to the end"), Player, keys(&["end"]), &act::SeekToEnd),
    held(tr_noop!("Previous frame"), keys(&[","]), &act::PreviousFrame),
    held(tr_noop!("Next frame"), keys(&["."]), &act::NextFrame),
    cmd(tr_noop!("Next program / Service ID"), Player, keys(&["x"]), &act::NextProgram),
    // VLC's key for the DVD menu; in a menu the arrows move and Enter (or Space) chooses.
    cmd(tr_noop!("Disc menu"), Player, keys(&["shift-m"]), &act::DiscMenu),
    cmd(tr_noop!("Choose in disc menu"), Player, keys(&["enter"]), &act::SelectInDiscMenu),
    // VLC's keys (S2.7.3); a held key keeps changing the speed.
    held(tr_noop!("Faster"), keys(&["]"]), &act::Faster),
    held(tr_noop!("Slower"), keys(&["["]), &act::Slower),
    cmd(tr_noop!("Normal speed"), Player, keys(&["="]), &act::NormalSpeed),
    held(tr_noop!("Increase volume"), keys(&["up"]), &act::VolumeUp),
    held(tr_noop!("Decrease volume"), keys(&["down"]), &act::VolumeDown),
    cmd(tr_noop!("Cycle audio track"), Player, keys(&["b"]), &act::CycleAudioTrack),
    cmd(tr_noop!("Cycle subtitle track"), Player, keys(&["v"]), &act::CycleSubtitleTrack),
    cmd(tr_noop!("Add subtitle file"), Player, NONE, &act::AddSubtitleFile),
    // VLC's keys; a held key keeps moving the audio.
    held(tr_noop!("Audio delay earlier"), keys(&["j"]), &act::AudioDelayEarlier),
    held(tr_noop!("Audio delay later"), keys(&["k"]), &act::AudioDelayLater),
    held(tr_noop!("Subtitle delay earlier"), keys(&["g"]), &act::SubtitleDelayEarlier),
    held(tr_noop!("Subtitle delay later"), keys(&["h"]), &act::SubtitleDelayLater),
    // VLC has no key for it; it lives in the View and video menus.
    cmd(tr_noop!("Picture in Picture"), Player, NONE, &act::PictureInPicture),
    cmd(tr_noop!("Take a snapshot"), Player, keys(&["shift-s"]), &act::Snapshot),
    cmd(tr_noop!("Record"), Player, keys(&["shift-r"]), &act::Record),
    // The context-menu key ("menu") exists on Windows and Linux keyboards.
    cmd(tr_noop!("Show context menu"), Player, keys(&["shift-f10", "menu"]), &act::ShowContextMenu),
    cmd(tr_noop!("Keyboard Shortcuts"), AppArea, NONE, &act::ShowKeyboardShortcuts),
    cmd(tr_noop!("Customize Player"), AppArea, NONE, &act::CustomizePlayer),
    cmd(
        tr_noop!("Settings"),
        AppArea,
        Keys::PerOs { mac: &["cmd-,"], others: &["ctrl-p", "ctrl-,"] },
        &act::OpenSettings,
    ),
    cmd(tr_noop!("About VLC"), AppArea, keys(&["shift-f1"]), &act::About),
    cmd(tr_noop!("Run Setup Again"), AppArea, NONE, &act::RunSetupAgain),
    cmd(tr_noop!("Copy Debug Info"), AppArea, NONE, &act::CopyDebugInfo),
    cmd(tr_noop!("Mark as Watched"), AppArea, NONE, &act::MarkWatched),
    cmd(tr_noop!("Mark as Unwatched"), AppArea, NONE, &act::MarkUnwatched),
    cmd(tr_noop!("Clear History"), AppArea, NONE, &act::ClearHistory),
    cmd(tr_noop!("Properties & Metadata"), AppArea, keys(&["secondary-i"]), &act::ShowProperties),
    cmd(tr_noop!("New Playlist"), AppArea, NONE, &act::NewPlaylist),
    cmd(tr_noop!("Save Queue as Playlist"), AppArea, NONE, &act::SaveQueueAsPlaylist),
    cmd(tr_noop!("Import Playlist"), AppArea, NONE, &act::ImportPlaylist),
    cmd(
        tr_noop!("Close Window"),
        Global,
        // Windows and Linux close windows with Alt+F4 through the OS.
        Keys::PerOs { mac: &["cmd-w"], others: &[] },
        &act::CloseWindow,
    ),
    cmd(tr_noop!("Quit"), Global, keys(&["secondary-q"]), &act::Quit),
];

/// The command that dispatches `action`, if it is one of ours.
pub fn find(action: &dyn Action) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.action.partial_eq(action))
}

/// The command labelled `label`.
pub fn by_label(label: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.label == label)
}
