//! The backend used when libVLC can't be loaded: every open fails with
//! [`ErrorKind::BackendUnavailable`], so the app still starts and can say why nothing plays.

use std::path::Path;

use super::audio::equalizer::Equalizer;
use super::audio::{OutputDevice, SpeakerSetup};
use super::backend::{Backend, BackendEvent, EventSender};
use super::colors::Colors;
use super::media_options::{MediaOptions, Picture, VideoFilters};
use super::state::{ErrorKind, PlaybackState};
use super::tracks::{TrackId, TrackKind};
use super::values::{BackendSeek, Generation, Offset, Rate, Start, StepDirection, Volume};
use crate::app::media::Media;

/// A backend that can't play anything.
pub struct UnavailableBackend {
    events: EventSender,
}

impl UnavailableBackend {
    /// A backend that answers every open through `events` with an error.
    pub fn new(events: EventSender) -> Self {
        Self { events }
    }
}

impl Backend for UnavailableBackend {
    fn open(&mut self, _: &Media, _: Start, generation: Generation) {
        let error = PlaybackState::Error(ErrorKind::BackendUnavailable);
        self.events.send(generation, BackendEvent::State(error));
    }

    fn play(&mut self) {}

    fn pause(&mut self) {}

    fn stop(&mut self) {}

    fn seek(&mut self, _: BackendSeek) {}

    fn frame_step(&mut self, _: StepDirection) {}

    fn set_rate(&mut self, _: Rate) {}

    fn set_volume(&mut self, _: Volume) {}

    fn set_mute(&mut self, _: bool) {}

    fn select_track(&mut self, _: TrackKind, _: Option<TrackId>) {}

    fn set_audio_delay(&mut self, _: Offset) {}

    fn set_subtitle_delay(&mut self, _: Offset) {}

    fn add_subtitle(&mut self, _: &Path) {}

    fn load_subtitle(&mut self, _: &Path) {}

    fn add_audio(&mut self, _: &Path) {}

    fn set_audio_device(&mut self, _: &OutputDevice) {}

    fn refresh_audio_devices(&mut self) {}

    fn set_speaker_setup(&mut self, _: SpeakerSetup) {}

    fn set_media_options(&mut self, _: &MediaOptions) {}

    fn set_equalizer(&mut self, _: Option<Equalizer>) {}

    fn set_picture(&mut self, _: Option<Picture>) {}

    fn set_video_filters(&mut self, _: VideoFilters) {}

    fn set_colors(&mut self, _: Colors) {}

    fn shut_down(self: Box<Self>) {}
}
