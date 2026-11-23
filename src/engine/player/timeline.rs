//! The [`Player`]'s side of the subtitle timeline (S6.6): when the lines of the subtitle showing
//! are on the media's clock, with its sync applied as it is shown.
//!
//! Only subtitle files the app reads (S6.2: SRT, WebVTT, ASS/SSA, MicroDVD) have known times,
//! styled ones included while libVLC draws them. libVLC 3 doesn't tell the times of a media's
//! own subtitle streams or of picture files, so those have no timeline.

use std::time::Duration;

use super::Player;
use crate::engine::cues::CueTrack;
use crate::engine::subtitle_sync::SubtitleSync;
use crate::engine::tracks::TrackKind;

/// When one subtitle line shows, in media time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CueSpan {
    /// When it appears.
    pub start: Duration,
    /// When it goes.
    pub end: Duration,
}

impl Player {
    /// Whether the times of the subtitle showing are known, so it can have a timeline.
    pub fn knows_subtitle_timings(&self) -> bool {
        self.showing_cues().is_some()
    }

    /// The lines of the subtitle showing at some time in `from..to` of the media, earliest
    /// first; `None` while their times aren't known. The delay, speed and duration factor apply
    /// as they do to what shows: the backend applies only the delay to the files it draws.
    pub fn subtitle_spans(&self, from: Duration, to: Duration) -> Option<Vec<CueSpan>> {
        let cues = self.showing_cues()?;
        let sync = self.shown_sync();
        let lines = cues.between(sync.file_time(from), sync.file_time(to), sync.duration_factor());
        let spans = lines.map(|(start, end)| CueSpan {
            start: sync.media_time(start),
            end: sync.media_time(end),
        });
        Some(spans.collect())
    }

    /// The cues of the subtitle file showing, whoever draws it.
    fn showing_cues(&self) -> Option<&CueTrack> {
        let track = self.info.tracks.selected(TrackKind::Subtitle)?;
        let file = self.info.subtitle_files.iter().find(|file| file.track == Some(track))?;
        file.cues.as_deref()
    }

    /// The selected subtitle's sync as it reaches what shows.
    fn shown_sync(&self) -> SubtitleSync {
        let sync = self.subtitle_sync();
        if self.sync_stretches() { sync } else { SubtitleSync::default().with_delay(sync.delay()) }
    }
}
