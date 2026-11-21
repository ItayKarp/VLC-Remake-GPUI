//! One GPUI action per user command (S0.5.1). What each one does, its label and its
//! default keys live in [`super::commands`].

use gpui::actions;

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
        /// Step one frame back.
        PreviousFrame,
        /// Step one frame forward.
        NextFrame,
        /// Switch to the next program (DVB service).
        NextProgram,
        /// Switch to the next subtitle track.
        CycleSubtitleTrack,
        /// Switch to the next audio track.
        CycleAudioTrack,
        /// Play a quarter faster.
        Faster,
        /// Play a quarter slower.
        Slower,
        /// Play at normal speed.
        NormalSpeed,
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
        /// Show or hide the queue.
        ToggleQueue,
        /// Show or hide the file browser.
        ToggleFileBrowser,
        /// Save the current frame as an image.
        Snapshot,
        /// Start or stop recording.
        Record,
    ]
);
