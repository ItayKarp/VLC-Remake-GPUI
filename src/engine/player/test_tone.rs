//! The [`Player`]'s side of the Stereo Mode's Test (S7.6.2): the backend plays a tone on the
//! left, then the right, at a fixed level beside the media. Media playing pauses for it, so the
//! tone is heard alone, and plays on once the tone is over, unless the user took over playback
//! meanwhile. Pressing Test again restarts the tone and its wait.

use std::time::Duration;

use gpui::Context;

use super::{Intent, Player};
use crate::engine::audio::test_tone;
use crate::engine::state::PlaybackState;

/// How long after the tone the media plays on, for the output's latency.
const RESUME_MARGIN: Duration = Duration::from_millis(300);

impl Player {
    /// Plays the test tone, pausing the media playing until it is over.
    pub fn play_test_tone(&mut self, cx: &mut Context<Self>) {
        if self.is_playing_for_user() {
            self.pause(cx);
            self.audio.paused_for_test = self.intent == Intent::Pause;
        }
        self.with_backend(|backend| backend.play_test_tone());
        self.audio.test_ends = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(test_tone::LENGTH + RESUME_MARGIN).await;
            // The player may be gone; nothing is left to resume then.
            let _ = this.update(cx, Self::test_tone_ended);
        }));
    }

    /// Whether the test tone is playing.
    pub fn is_testing_tone(&self) -> bool {
        self.audio.test_ends.is_some()
    }

    /// Plays on the media the test paused, if the user left it paused.
    fn test_tone_ended(&mut self, cx: &mut Context<Self>) {
        self.audio.test_ends = None;
        let paused = matches!(self.state, PlaybackState::Paused | PlaybackState::Buffering);
        if std::mem::take(&mut self.audio.paused_for_test) && paused {
            self.play(cx);
        }
        cx.notify();
    }
}
