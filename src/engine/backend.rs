//! The interface between the [`Player`](super::Player) and a playback backend (libVLC, or the
//! [`mock`](super::mock) in tests), and the channel backend events travel through (S1.2.4).
//!
//! Backends report from their own threads, inside their own callbacks. Those callbacks only
//! [`EventSender::send`] an event and return: the player handles it later on the GPUI foreground
//! thread. That is what keeps commands out of libVLC's event callbacks, where calling back into
//! libVLC deadlocks.

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::channel::mpsc;

use super::audio::{AudioDevice, OutputDevice, SpeakerSetup};
use super::media_info::{Chapter, Metadata, VideoSize};
use super::media_options::MediaOptions;
use super::state::PlaybackState;
use super::tracks::{TrackId, TrackKind, Tracks};
use super::values::{BackendSeek, Generation, Offset, Rate, Start, StepDirection, Volume};
use crate::app::media::Media;

/// Something the backend observed about the media it is playing.
#[derive(Clone, Debug, PartialEq)]
pub enum BackendEvent {
    /// Playback moved to this state. Backends report `Buffering` here too, and the fill level
    /// with [`Self::Buffering`].
    State(PlaybackState),
    /// The buffer is this full, 0.0 to 100.0.
    Buffering(f32),
    /// The playback position. Backends may report it as often as they like.
    Position(Duration),
    /// The media's length; `None` while unknown (live streams, or a backend reporting 0 or -1).
    Duration(Option<Duration>),
    /// Whether seeking works.
    Seekable(bool),
    /// Whether pausing works.
    Pausable(bool),
    /// The track list or selection changed.
    Tracks(Tracks),
    /// The displayed video size changed; `None` when there is no video.
    VideoSize(Option<VideoSize>),
    /// The chapter list changed.
    Chapters(Vec<Chapter>),
    /// The metadata tags changed.
    Metadata(Metadata),
    /// The last [`Backend::seek`] finished.
    SeekCompleted,
    /// The last [`Backend::frame_step`] finished.
    FrameStepped,
    /// The output devices plugged in now, without the system default; empty when there are
    /// none. Sent when the list changes, and once at the start.
    AudioDevices(Vec<AudioDevice>),
    /// The OS asked for this app's audio to stop (`true`: another app, such as a call, needs
    /// it) or allowed it again (`false`).
    Corked(bool),
}

impl BackendEvent {
    /// Whether the event is about the open media, so one from an earlier open is stale.
    /// Device and audio-focus events are about the machine, and always apply.
    pub fn is_about_media(&self) -> bool {
        !matches!(self, Self::AudioDevices(_) | Self::Corked(_))
    }
}

/// A backend event, labelled with the open it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Tagged {
    /// The generation passed to the [`Backend::open`] the event is about.
    pub generation: Generation,
    /// What happened.
    pub event: BackendEvent,
}

/// Sends backend events to the player. Cheap to clone, never blocks, and safe to call from any
/// thread, including inside a libVLC callback.
#[derive(Clone, Debug)]
pub struct EventSender(mpsc::UnboundedSender<Tagged>);

impl EventSender {
    /// Queues `event` for the player. Does nothing once the player is gone.
    pub fn send(&self, generation: Generation, event: BackendEvent) {
        // An error only means the player was dropped; nobody is left to tell.
        let _ = self.0.unbounded_send(Tagged { generation, event });
    }
}

/// The player's end of the event channel.
pub type EventReceiver = mpsc::UnboundedReceiver<Tagged>;

/// A new event channel.
pub fn channel() -> (EventSender, EventReceiver) {
    let (sender, receiver) = mpsc::unbounded();
    (EventSender(sender), receiver)
}

/// A backend command as a value: what the mock records, and what the libVLC backend queues for
/// its worker thread.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Open `media` for the open with `generation`, starting as `start` says.
    Open { media: Media, start: Start, generation: Generation },
    /// Play.
    Play,
    /// Pause.
    Pause,
    /// Stop.
    Stop,
    /// Seek.
    Seek(BackendSeek),
    /// Step one frame.
    FrameStep(StepDirection),
    /// Set the rate.
    SetRate(Rate),
    /// Set the volume.
    SetVolume(Volume),
    /// Mute or unmute.
    SetMute(bool),
    /// Select a track.
    SelectTrack(TrackKind, Option<TrackId>),
    /// Set the audio delay.
    SetAudioDelay(Offset),
    /// Set the subtitle delay.
    SetSubtitleDelay(Offset),
    /// Add a subtitle file to the open media and show it.
    AddSubtitle(PathBuf),
    /// Add an audio file to the open media as a track and play it.
    AddAudio(PathBuf),
    /// Play through this device.
    SetAudioDevice(OutputDevice),
    /// List the output devices again.
    RefreshAudioDevices,
    /// Set up the speakers.
    SetSpeakerSetup(SpeakerSetup),
    /// Open later media with these options.
    SetMediaOptions(MediaOptions),
}

/// A playback backend.
///
/// The contract every implementation keeps:
/// - Commands return quickly and report their outcome as events through the [`EventSender`] it
///   was built with, tagged with the generation of the latest [`Self::open`].
/// - Any command may arrive in any state; one that makes no sense there is ignored.
/// - Every [`Self::seek`] is answered by exactly one [`BackendEvent::SeekCompleted`], and every
///   [`Self::frame_step`] by one [`BackendEvent::FrameStepped`], unless an open, stop, end or
///   error comes first.
/// - The device list ([`BackendEvent::AudioDevices`]) is reported once soon after creation and
///   again whenever it changes, whether or not media is open.
/// - [`Self::shut_down`] may block; the player runs it with a timeout.
pub trait Backend: Send {
    /// Starts opening `media` and playing it (or holding it paused) from where `start` says,
    /// abandoning whatever was open.
    fn open(&mut self, media: &Media, start: Start, generation: Generation);
    /// Resumes or starts playback.
    fn play(&mut self);
    /// Pauses playback.
    fn pause(&mut self);
    /// Stops playback and unloads the media's decoders.
    fn stop(&mut self);
    /// Moves the playback position.
    fn seek(&mut self, to: BackendSeek);
    /// Pauses and shows the next or previous frame.
    fn frame_step(&mut self, direction: StepDirection);
    /// Sets the playback speed, with audio pitch correction.
    fn set_rate(&mut self, rate: Rate);
    /// Sets the output volume.
    fn set_volume(&mut self, volume: Volume);
    /// Mutes or unmutes the output.
    fn set_mute(&mut self, muted: bool);
    /// Plays `track` for `kind`, or turns that kind off with `None`.
    fn select_track(&mut self, kind: TrackKind, track: Option<TrackId>);
    /// Delays audio against video (negative plays it earlier).
    fn set_audio_delay(&mut self, delay: Offset);
    /// Delays subtitles against video (negative shows them earlier).
    fn set_subtitle_delay(&mut self, delay: Offset);
    /// Adds the subtitle file at `path` to the open media and shows it. Its track appears in a
    /// later [`BackendEvent::Tracks`].
    fn add_subtitle(&mut self, path: &Path);
    /// Adds the audio file at `path` to the open media as a track and plays it (S4.8.1). Its
    /// track appears in a later [`BackendEvent::Tracks`].
    fn add_audio(&mut self, path: &Path);
    /// Plays through `device` from now on, for this and later media.
    fn set_audio_device(&mut self, device: &OutputDevice);
    /// Lists the output devices again now, rather than at the next poll; a changed list is
    /// reported as usual.
    fn refresh_audio_devices(&mut self);
    /// Mixes channels for `setup` from now on, for this and later media.
    fn set_speaker_setup(&mut self, setup: SpeakerSetup);
    /// Opens media with `options` from the next [`Self::open`] on; the open media keeps its own.
    fn set_media_options(&mut self, options: &MediaOptions);
    /// Stops playback and releases everything. May block on a hung source.
    fn shut_down(self: Box<Self>);
}
