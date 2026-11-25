//! The playback state machine (S1.2.2): the states a [`Player`](super::Player) moves through and
//! the transitions it allows between them.
//!
//! ```text
//!            ┌────────────────────────── open (from any state) ──────────────────────────┐
//!            ▼                                                                           │
//! Idle ─► Opening ─► Buffering ⇄ Playing ⇄ Paused        (and Buffering ⇄ Paused)        │
//!            │           │           │         │                                         │
//!            └───────────┴───────────┴─────────┴─► Ended │ Stopped │ Error(kind) ────────┘
//! ```
//!
//! | From        | To                                                            |
//! |-------------|---------------------------------------------------------------|
//! | any state   | `Opening` (a new open cancels the current one)                |
//! | `Opening`   | `Buffering`, `Playing`, `Paused`, `Ended`, `Stopped`, `Error` |
//! | `Buffering` | `Playing`, `Paused`, `Ended`, `Stopped`, `Error`              |
//! | `Playing`   | `Buffering`, `Paused`, `Ended`, `Stopped`, `Error`            |
//! | `Paused`    | `Buffering`, `Playing`, `Ended`, `Stopped`, `Error`           |
//! | `Ended`     | `Stopped`                                                     |
//!
//! `Opening → Ended` covers media with nothing to play for (an empty stream). A state never
//! "transitions" to itself: repeating the current state is not an edge, which is what makes
//! `Ended` arrive once.

/// Why playback failed. Each kind maps to its own message in the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The file or folder doesn't exist.
    NotFound,
    /// The file is empty (zero bytes).
    EmptyFile,
    /// A folder, pipe or device was given where a media file was expected.
    NotAFile,
    /// The file can't be read: no permission, or a symlink loop.
    Unreadable,
    /// Another process holds the file locked (a Windows sharing violation).
    Locked,
    /// The backend couldn't open the media, for a reason it didn't narrow down.
    OpenFailed,
    /// The media has no audio, video or subtitle stream the backend can play.
    NoPlayableStreams,
    /// The media is damaged past the point the backend can continue.
    Corrupt,
    /// The media is DRM-protected (FairPlay, Widevine).
    Protected,
    /// libVLC couldn't be loaded, so nothing can play.
    BackendUnavailable,
    /// The source went away while playing: the file was deleted, its drive unplugged or the
    /// network share dropped.
    SourceLost,
    /// A link whose scheme isn't media's (`javascript:`, `data:`, …), refused before libVLC
    /// sees it (S9.7.2).
    RefusedLink,
}

/// Where playback is. See the module comment for the allowed transitions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PlaybackState {
    /// Nothing has been opened yet.
    #[default]
    Idle,
    /// Media is being opened and probed.
    Opening,
    /// Playback waits for data (network, or after a seek).
    Buffering,
    /// Playback is running.
    Playing,
    /// Playback is paused.
    Paused,
    /// Playback reached the end of the media.
    Ended,
    /// The user stopped playback.
    Stopped,
    /// Playback failed.
    Error(ErrorKind),
}

impl PlaybackState {
    /// Whether the machine may move from `self` to `next`. Staying in the same state is not a
    /// transition, so this is `false` when they are equal (except for a new `Opening`).
    pub fn can_become(self, next: Self) -> bool {
        use PlaybackState::{Buffering, Ended, Error, Opening, Paused, Playing, Stopped};
        matches!(
            (self, next),
            (_, Opening)
                | (Opening, Buffering | Playing | Paused)
                | (Buffering, Playing | Paused)
                | (Playing, Buffering | Paused)
                | (Paused, Buffering | Playing)
                | (Ended, Stopped)
                | (Opening | Buffering | Playing | Paused, Ended | Stopped | Error(_))
        )
    }

    /// Whether media is loaded and the backend is working on it: `Opening` through `Paused`.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Opening | Self::Buffering | Self::Playing | Self::Paused)
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorKind::{Corrupt, SourceLost};
    use super::PlaybackState::{self, *};

    /// One of each state; `Error` twice, so `Error(a) → Error(b)` is covered too.
    const STATES: [PlaybackState; 9] = [
        Idle,
        Opening,
        Buffering,
        Playing,
        Paused,
        Ended,
        Stopped,
        Error(Corrupt),
        Error(SourceLost),
    ];

    /// Every edge in the graph, as documented in the module comment.
    fn is_edge(from: PlaybackState, to: PlaybackState) -> bool {
        let active = [Opening, Buffering, Playing, Paused];
        let ending = [Ended, Stopped, Error(Corrupt), Error(SourceLost)];
        let forward = [
            (Opening, Buffering),
            (Opening, Playing),
            (Opening, Paused),
            (Buffering, Playing),
            (Buffering, Paused),
            (Playing, Buffering),
            (Playing, Paused),
            (Paused, Buffering),
            (Paused, Playing),
            (Ended, Stopped),
        ];
        to == Opening
            || forward.contains(&(from, to))
            || (active.contains(&from) && ending.contains(&to))
    }

    #[test]
    fn every_pair_of_states_matches_the_documented_graph() {
        for from in STATES {
            for to in STATES {
                assert_eq!(from.can_become(to), is_edge(from, to), "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn terminal_states_only_reopen() {
        for from in [Idle, Stopped, Error(Corrupt)] {
            let next: Vec<_> = STATES.into_iter().filter(|&to| from.can_become(to)).collect();
            assert_eq!(next, [Opening], "{from:?}");
        }
    }

    #[test]
    fn ended_is_not_reached_twice() {
        assert!(!Ended.can_become(Ended));
    }

    #[test]
    fn only_loaded_states_are_active() {
        let active: Vec<_> = STATES.into_iter().filter(|state| state.is_active()).collect();
        assert_eq!(active, [Opening, Buffering, Playing, Paused]);
    }
}
