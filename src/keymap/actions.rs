//! One GPUI action per user command (S0.5.1). What each one does, its label and its
//! default keys live in [`super::commands`].

use gpui::{Action, actions};

/// A command chosen from the macOS menu bar that has a plain-key shortcut (Space, S, F, …).
/// The bar can't list those commands directly: GPUI would give the item the key as its key
/// equivalent, and macOS fires key equivalents even while a text field has focus. This action
/// has no keys, so the item has none.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = player, no_json, no_register)]
pub struct FromMenuBar {
    /// The command's label in [`super::commands::COMMANDS`].
    pub command: &'static str,
}

actions!(
    vlc,
    [
        /// Quit the app. Unsaved onboarding choices are discarded.
        Quit,
        /// Close the active window.
        CloseWindow,
        /// Run onboarding again (S0.2.5).
        RunSetupAgain,
        /// Choose media files to open.
        OpenFile,
        /// Choose a folder to open.
        OpenFolder,
        /// Open the file paths or URLs on the clipboard.
        OpenFromClipboard,
        /// Open a network stream by URL.
        OpenNetworkStream,
        /// Open a DVD, Blu-ray or audio CD.
        OpenDisc,
        /// Open a camera or capture card.
        OpenCaptureDevice,
        /// Convert or save media to another format.
        Convert,
        /// Open the Settings window.
        OpenSettings,
        /// Show the Keyboard Shortcuts dialog.
        ShowKeyboardShortcuts,
        /// Customize the player layout.
        CustomizePlayer,
        /// Show the About dialog.
        About,
        /// Copy versions, OS, GPU and settings (without paths) for a bug report.
        CopyDebugInfo,
        /// Search the library.
        Search,
        /// Mark the current media watched: it plays from the start next time.
        MarkWatched,
        /// Mark the current media unwatched, forgetting where it was left.
        MarkUnwatched,
        /// Forget the playback history: positions, plays and the recent list.
        ClearHistory,
        /// Show the current media's tags and details (S4.3.5).
        ShowProperties,
        /// Open the New Playlist dialog (S3.5.1).
        NewPlaylist,
        /// Open the New Playlist dialog holding the queue's items (S3.5.5).
        SaveQueueAsPlaylist,
        /// Save playlist files (`.m3u`, `.pls`, `.xspf`, …) as saved playlists (S3.6.2).
        ImportPlaylist,
    ]
);

actions!(
    player,
    [
        /// Play or pause.
        PlayPause,
        /// Stop playback.
        Stop,
        /// Seek backward by the short skip (10 s).
        SeekBackward,
        /// Seek forward by the short skip (10 s).
        SeekForward,
        /// Seek backward by the very short skip (3 s).
        SeekBackwardVeryShort,
        /// Seek forward by the very short skip (3 s).
        SeekForwardVeryShort,
        /// Seek backward by the medium skip (1 min).
        SeekBackwardMedium,
        /// Seek forward by the medium skip (1 min).
        SeekForwardMedium,
        /// Seek backward by the long skip (5 min).
        SeekBackwardLong,
        /// Seek forward by the long skip (5 min).
        SeekForwardLong,
        /// Go to the start of the media.
        SeekToStart,
        /// Go to the end of the media.
        SeekToEnd,
        /// Go to the previous item in the queue.
        PreviousItem,
        /// Go to the next item in the queue.
        NextItem,
        /// Go back to the start of the chapter, or to the previous one (S8.2.3).
        PreviousChapter,
        /// Go to the next chapter (S8.2.3).
        NextChapter,
        /// Bookmark where playback is (S8.3.2).
        AddBookmark,
        /// Move Repeat to its next mode: Off, All, One.
        CycleRepeat,
        /// Switch Shuffle on or off.
        ToggleShuffle,
        /// Step one frame back.
        PreviousFrame,
        /// Step one frame forward.
        NextFrame,
        /// Switch to the next program (DVB service).
        NextProgram,
        /// Switch to the next subtitle track.
        CycleSubtitleTrack,
        /// Ask for subtitle files to add to the playing media (S6.1.4).
        AddSubtitleFile,
        /// Switch to the next audio track.
        CycleAudioTrack,
        /// Play a quarter faster.
        Faster,
        /// Play a quarter slower.
        Slower,
        /// Play at normal speed.
        NormalSpeed,
        /// Play the audio 50 ms sooner against the video (S4.8.1).
        AudioDelayEarlier,
        /// Play the audio 50 ms later against the video (S4.8.1).
        AudioDelayLater,
        /// Show the subtitles 50 ms sooner against the video (S6.3.2).
        SubtitleDelayEarlier,
        /// Show the subtitles 50 ms later against the video (S6.3.2).
        SubtitleDelayLater,
        /// Raise the volume.
        VolumeUp,
        /// Lower the volume.
        VolumeDown,
        /// Enter or leave fullscreen.
        ToggleFullscreen,
        /// Leave fullscreen.
        ExitFullscreen,
        /// Show the playback position and time.
        ShowTime,
        /// Show the controls while in fullscreen.
        ShowControls,
        /// Hide or show the whole interface around the video.
        HideInterface,
        /// Pop the player out into the Picture in Picture window, or back in (S8.5).
        PictureInPicture,
        /// Show or hide the queue.
        ToggleQueue,
        /// Show or hide the file browser.
        ToggleFileBrowser,
        /// Go back to the library's previous place (S4.5.2).
        GoBack,
        /// Go forward to the library place gone back from (S4.5.2).
        GoForward,
        /// Save the current frame as an image.
        Snapshot,
        /// Start or stop recording.
        Record,
        /// Open the video's right-click menu from the keyboard, at the video.
        ShowContextMenu,
    ]
);
