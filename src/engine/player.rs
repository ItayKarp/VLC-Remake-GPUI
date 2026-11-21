//! The [`Player`] GPUI entity (S1.2.1): owns a backend, runs the playback state machine, and
//! turns backend events into [`PlayerEvent`]s for the UI.

mod audio;
mod commands;
mod receive;
mod tracks;

use std::time::Duration;

use futures::StreamExt as _;
use gpui::{AsyncApp, Context, EventEmitter, Subscription, Task, WeakEntity};

use self::audio::AudioState;
use self::tracks::TrackChoices;
use super::audio::AudioDevice;
use super::backend::{self, Backend, EventReceiver, EventSender};
use super::coalesce::{FrameSteps, SeekQueue};
use super::media_info::{Chapter, Metadata, VideoSize};
use super::position::PositionTracker;
use super::shutdown::{self, SHUTDOWN_TIMEOUT, Shutdown};
use super::state::{ErrorKind, PlaybackState};
use super::tracks::Tracks;
use super::values::{Generation, Offset, Rate, Volume};
use crate::app::media::Media;

/// What the player tells the UI. After `StateChanged(Opening)` the previous media is forgotten:
/// read the player's getters again rather than keeping old values.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    /// The playback state changed.
    StateChanged(PlaybackState),
    /// The shown position changed; sent about 15 times a second at most, and at once on a seek.
    PositionChanged(Duration),
    /// The media's length became known, changed, or became unknown (`None`).
    DurationChanged(Option<Duration>),
    /// Whether seeking works changed.
    SeekableChanged(bool),
    /// Whether pausing works changed.
    PausableChanged(bool),
    /// The track list or selection changed; see [`Player::tracks`].
    TracksChanged,
    /// The buffer is this full, 0.0 to 100.0.
    Buffering(f32),
    /// The displayed video size changed; `None` when there is no video.
    VideoSizeChanged(Option<VideoSize>),
    /// The chapter list changed; see [`Player::chapters`].
    ChaptersChanged,
    /// The metadata changed; see [`Player::metadata`].
    MetadataChanged,
    /// Playback reached the end. Sent once per open, right after `StateChanged(Ended)`.
    EndReached,
    /// Playback failed. Sent right after `StateChanged(Error(kind))`.
    Error(ErrorKind),
    /// The volume or mute changed; see [`Player::volume`] and [`Player::is_muted`].
    VolumeChanged,
    /// The output devices changed; see [`Player::audio_devices`].
    AudioDevicesChanged,
    /// The device sound went to disappeared; sound moved to the system default (and playback
    /// paused, if the policy says so).
    AudioDeviceLost(AudioDevice),
}

/// Whether the user wants the media playing or paused, honored once the backend can act on it
/// (a pause asked for while buffering takes effect when buffering ends).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Intent {
    #[default]
    Play,
    Pause,
}

/// The user's playback settings, kept across opens.
#[derive(Clone, Copy, Debug, Default)]
struct Controls {
    rate: Rate,
    volume: Volume,
    muted: bool,
    audio_delay: Offset,
    subtitle_delay: Offset,
}

/// What the backend reported about the open media.
#[derive(Clone, Debug, Default)]
struct Info {
    pausable: Option<bool>,
    /// How full the input cache is, 0.0 to 100.0.
    buffer_fill: f32,
    tracks: Tracks,
    video_size: Option<VideoSize>,
    chapters: Vec<Chapter>,
    metadata: Metadata,
}

/// The playback engine as a GPUI entity. Commands are methods; changes arrive as
/// [`PlayerEvent`]s.
pub struct Player {
    /// `None` once shut down; every command is then ignored.
    backend: Option<Box<dyn Backend>>,
    generation: Generation,
    state: PlaybackState,
    intent: Intent,
    media: Option<Media>,
    controls: Controls,
    info: Info,
    audio: AudioState,
    track_choices: TrackChoices,
    position: PositionTracker,
    seeks: SeekQueue,
    steps: FrameSteps,
    _pump: Task<()>,
    _quit: Subscription,
}

impl EventEmitter<PlayerEvent> for Player {}

impl Player {
    /// A player driving the backend `make_backend` builds around the event sender it is given.
    /// The backend is shut down (with a timeout) when the app quits.
    pub fn new(
        make_backend: impl FnOnce(EventSender) -> Box<dyn Backend>,
        cx: &mut Context<Self>,
    ) -> Self {
        let (sender, receiver) = backend::channel();
        Self {
            backend: Some(make_backend(sender)),
            generation: Generation::default(),
            state: PlaybackState::Idle,
            intent: Intent::Play,
            media: None,
            controls: Controls::default(),
            info: Info::default(),
            audio: AudioState::default(),
            track_choices: TrackChoices::default(),
            position: PositionTracker::default(),
            seeks: SeekQueue::default(),
            steps: FrameSteps::default(),
            _pump: cx.spawn(async move |this, cx| pump(this, receiver, cx).await),
            _quit: cx.on_app_quit(|this, _| {
                this.shut_down();
                async {}
            }),
        }
    }

    /// The playback state.
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Whether the user wants the media playing rather than paused. A pause asked for while
    /// opening or buffering shows here before the state follows, so the Play/Pause button never
    /// lags behind a click (S2.2.2).
    pub fn wants_to_play(&self) -> bool {
        self.intent == Intent::Play
    }

    /// The media opened last, if any.
    pub fn media(&self) -> Option<&Media> {
        self.media.as_ref()
    }

    /// The position the UI should show.
    pub fn position(&self) -> Duration {
        self.position.shown()
    }

    /// The media's length, or `None` while unknown (the UI shows `--:--`).
    pub fn duration(&self) -> Option<Duration> {
        self.position.duration()
    }

    /// Whether seeking works; `None` until the backend says.
    pub fn is_seekable(&self) -> Option<bool> {
        self.seeks.is_seekable()
    }

    /// Whether pausing works; `None` until the backend says.
    pub fn is_pausable(&self) -> Option<bool> {
        self.info.pausable
    }

    /// How full the input cache is, 0.0 to 100.0, as the backend last said; 0 until it does.
    pub fn buffer_fill(&self) -> f32 {
        self.info.buffer_fill
    }

    /// The playback speed.
    pub fn rate(&self) -> Rate {
        self.controls.rate
    }

    /// The output volume.
    pub fn volume(&self) -> Volume {
        self.controls.volume
    }

    /// Whether the output is muted.
    pub fn is_muted(&self) -> bool {
        self.controls.muted
    }

    /// The audio delay against video.
    pub fn audio_delay(&self) -> Offset {
        self.controls.audio_delay
    }

    /// The subtitle delay against video.
    pub fn subtitle_delay(&self) -> Offset {
        self.controls.subtitle_delay
    }

    /// The media's tracks and the selected ones.
    pub fn tracks(&self) -> &Tracks {
        &self.info.tracks
    }

    /// The displayed video size, or `None` without video.
    pub fn video_size(&self) -> Option<VideoSize> {
        self.info.video_size
    }

    /// The media's chapters.
    pub fn chapters(&self) -> &[Chapter] {
        &self.info.chapters
    }

    /// The media's metadata tags.
    pub fn metadata(&self) -> &Metadata {
        &self.info.metadata
    }

    /// Stops and releases the backend, waiting at most [`SHUTDOWN_TIMEOUT`]. Later commands are
    /// ignored. Returns `None` when already shut down.
    pub fn shut_down(&mut self) -> Option<Shutdown> {
        let backend = self.backend.take()?;
        Some(shutdown::shut_down(backend, SHUTDOWN_TIMEOUT))
    }

    /// Runs `command` on the backend, unless it was shut down.
    fn with_backend(&mut self, command: impl FnOnce(&mut dyn Backend)) {
        if let Some(backend) = self.backend.as_deref_mut() {
            command(backend);
        }
    }
}

/// Hands backend events to the player on the foreground thread, until either side is gone.
async fn pump(this: WeakEntity<Player>, mut receiver: EventReceiver, cx: &mut AsyncApp) {
    while let Some(tagged) = receiver.next().await {
        if this.update(cx, |player, cx| player.receive(tagged, cx)).is_err() {
            break;
        }
    }
}
