//! Showing the picture of video that opens paused: the last session's media restored (S3.4),
//! and a paused video reopened for new filters (S7.2, S7.3).
//!
//! Both get a new video output, and libVLC 3 draws nothing into a new output while the media is
//! paused: `start-paused` pauses it before a picture is decoded, and neither a seek nor a frame
//! step draws one then. So the media plays, muted, until `vmem` shows its first picture, and is
//! paused there. The player never sees it play: the callback keeps reporting it paused
//! ([`super::events`]).

use std::ffi::c_int;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use super::worker::{Message, Worker};

/// How long a picture may take before the media is paused anyway.
const HOLD_LIMIT: Duration = Duration::from_secs(3);

/// A paused media playing, muted, until its first picture shows.
#[derive(Clone, Copy, Debug)]
pub struct Hold {
    /// Whether the user had it muted, for when the hold ends; `None` when libVLC didn't say.
    muted: Option<bool>,
    /// When it is paused even without a picture.
    until: Instant,
}

impl Worker {
    /// Plays the media being reopened muted until its first picture shows, then pauses it
    /// there. Only where video goes to the app, which then has something to show.
    pub(super) fn hold_first_picture(&mut self) {
        let Some(video) = &self.video else { return };
        let worker = self.shared.worker.clone();
        video.after_next_picture(Box::new(move || {
            // An error means the worker is gone (shut down); nothing is left to pause.
            let _ = worker.send(Message::PictureShown);
        }));
        let api = &self.vlc.api;
        // SAFETY: the player is live.
        let muted = unsafe { (api.libvlc_audio_get_mute)(self.handles.player) };
        // SAFETY: the player is live.
        unsafe { (api.libvlc_audio_set_mute)(self.handles.player, 1) };
        self.shared.holding.store(true, Ordering::Relaxed);
        let muted = (muted >= 0).then_some(muted != 0);
        self.hold = Some(Hold { muted, until: Instant::now() + HOLD_LIMIT });
    }

    /// Whether the media opening now will show a picture: video is decoded and its parse found
    /// a video track. Media without one would play, muted, until the hold gave up.
    pub(super) fn expects_picture(&self) -> bool {
        self.media_options.video && self.video.as_ref().is_some_and(|video| video.knows_display())
    }

    /// Pauses the held media where its first picture showed, and unmutes it.
    pub(super) fn pause_held(&mut self) {
        let Some(hold) = self.hold.take() else { return };
        // SAFETY: the player is live. Its `Paused` event ends the hold for the callback.
        unsafe { (self.vlc.api.libvlc_media_player_set_pause)(self.handles.player, 1) };
        self.restore_mute(hold);
    }

    /// Ends the hold without pausing (the user played, stopped or opened other media), and
    /// unmutes; `false` if nothing was held.
    pub(super) fn end_hold(&mut self) -> bool {
        let Some(hold) = self.hold.take() else { return false };
        if let Some(video) = &self.video {
            video.forget_next_picture();
        }
        self.shared.holding.store(false, Ordering::Relaxed);
        self.restore_mute(hold);
        true
    }

    /// Keeps a mute the user asked for while holding for when the hold ends; `false` if
    /// nothing is held, so it applies now.
    pub(super) fn hold_mute(&mut self, muted: bool) -> bool {
        let Some(hold) = &mut self.hold else { return false };
        hold.muted = Some(muted);
        true
    }

    /// Pauses the held media if its picture is taking too long.
    pub(super) fn pause_held_if_due(&mut self, now: Instant) {
        if self.hold.is_some_and(|hold| now >= hold.until) {
            tracing::warn!("libvlc: no picture after reopening paused video; pausing anyway");
            self.pause_held();
        }
    }

    /// How long until [`Self::pause_held_if_due`] pauses, while holding.
    pub(super) fn until_hold_ends(&self, now: Instant) -> Option<Duration> {
        self.hold.map(|hold| hold.until.saturating_duration_since(now))
    }

    fn restore_mute(&self, hold: Hold) {
        let muted = hold.muted.unwrap_or_default();
        // SAFETY: the player is live.
        unsafe { (self.vlc.api.libvlc_audio_set_mute)(self.handles.player, c_int::from(muted)) };
    }
}
