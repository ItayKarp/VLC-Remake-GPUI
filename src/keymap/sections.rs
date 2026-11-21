//! How the Keyboard Shortcuts dialog (S2.10.4) groups the commands, top to bottom. The first
//! two follow the design (`dialogs/keyboard_shortcuts/keyboard_shortcuts.png`); the rest are
//! ours. Every command in [`super::commands::COMMANDS`] is in exactly one section.

use gpui::Action;

use super::actions as act;

/// One titled group of commands.
pub struct Section {
    /// The heading above the group.
    pub title: &'static str,
    /// The group's commands, in order.
    pub actions: &'static [&'static dyn Action],
}

/// The dialog's sections, in order.
pub const SECTIONS: &[Section] = &[
    Section {
        title: "Basic",
        actions: &[
            &act::ToggleFullscreen,
            &act::ExitFullscreen,
            &act::ShowTime,
            &act::ShowControls,
            &act::HideInterface,
            &act::ShowContextMenu,
            &act::ToggleQueue,
            &act::ToggleFileBrowser,
        ],
    },
    Section {
        title: "Playback",
        actions: &[
            &act::PlayPause,
            &act::Stop,
            &act::NextItem,
            &act::PreviousItem,
            &act::CycleRepeat,
            &act::ToggleShuffle,
            &act::NextProgram,
            &act::PreviousFrame,
            &act::NextFrame,
            &act::Faster,
            &act::Slower,
            &act::NormalSpeed,
        ],
    },
    Section {
        title: "Seeking",
        actions: &[
            &act::SeekBackward,
            &act::SeekForward,
            &act::SeekBackwardVeryShort,
            &act::SeekForwardVeryShort,
            &act::SeekBackwardMedium,
            &act::SeekForwardMedium,
            &act::SeekBackwardLong,
            &act::SeekForwardLong,
            &act::SeekToStart,
            &act::SeekToEnd,
        ],
    },
    Section {
        title: "Audio and Subtitles",
        actions: &[
            &act::VolumeUp,
            &act::VolumeDown,
            &act::CycleAudioTrack,
            &act::CycleSubtitleTrack,
        ],
    },
    Section { title: "Capture", actions: &[&act::Snapshot, &act::Record] },
    Section {
        title: "Open",
        actions: &[
            &act::OpenFile,
            &act::OpenFolder,
            &act::OpenFromClipboard,
            &act::OpenNetworkStream,
            &act::OpenDisc,
            &act::OpenCaptureDevice,
            &act::Convert,
        ],
    },
    Section {
        title: "Library",
        actions: &[
            &act::Search,
            &act::MarkWatched,
            &act::MarkUnwatched,
            &act::ClearHistory,
            &act::ShowProperties,
            &act::NewPlaylist,
            &act::SaveQueueAsPlaylist,
            &act::ImportPlaylist,
        ],
    },
    Section {
        title: "Application",
        actions: &[
            &act::ShowKeyboardShortcuts,
            &act::CustomizePlayer,
            &act::OpenSettings,
            &act::About,
            &act::RunSetupAgain,
            &act::CopyDebugInfo,
            &act::CloseWindow,
            &act::Quit,
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::commands::COMMANDS;

    #[test]
    fn every_command_is_in_exactly_one_section() {
        for command in COMMANDS {
            let sections = SECTIONS
                .iter()
                .filter(|section| section.actions.iter().any(|a| a.partial_eq(command.action)))
                .count();
            assert_eq!(sections, 1, "{} is in {sections} sections", command.label);
        }
        let listed: usize = SECTIONS.iter().map(|section| section.actions.len()).sum();
        assert_eq!(listed, COMMANDS.len(), "a section lists something that isn't a command");
    }
}
