//! The main window's side of the picture's adjustments (S7.1): it gives the player the picture in
//! effect, Settings → Video's with the settings panel's session values over it, and follows
//! both. While a slider is dragged the player gets at most about 30 changes a second (S7.1.6),
//! the last one always arriving, so the video doesn't stutter under a flood of filter changes.

use std::time::{Duration, Instant};

use gpui::{App, Context, Entity, Subscription, Task};

use super::MainWindow;
use crate::effects::{self, EffectSession};
use crate::engine::media_options::Picture;
use crate::engine::player::Player;
use crate::settings::SettingsStore;
use crate::ui::slider::Throttle;

/// The shortest time between two changes the player is given: about 30 a second.
const APPLY_INTERVAL: Duration = Duration::from_millis(33);

/// What the window keeps to apply the picture.
pub(super) struct PictureWatch {
    /// Paces the changes.
    throttle: Throttle<Option<Picture>>,
    /// Sends the change held back, once the interval is over.
    flush: Option<Task<()>>,
    /// Follow the settings and the session.
    _watch: Option<[Subscription; 2]>,
}

impl PictureWatch {
    /// Paces the changes `watch` sees.
    fn following(watch: [Subscription; 2]) -> Self {
        Self { throttle: Throttle::new(APPLY_INTERVAL), flush: None, _watch: Some(watch) }
    }
}

impl Default for PictureWatch {
    fn default() -> Self {
        Self { throttle: Throttle::new(APPLY_INTERVAL), flush: None, _watch: None }
    }
}

impl MainWindow {
    /// Gives a new `player` the picture in effect, and follows later changes to it.
    pub(super) fn set_up_picture(&mut self, player: &Entity<Player>, cx: &mut Context<Self>) {
        let picture = picture(cx);
        player.update(cx, |player, _| player.set_picture(picture));
        self.tracks.picture = PictureWatch::following([
            cx.observe_global::<SettingsStore>(Self::offer_picture),
            cx.observe_global::<EffectSession>(Self::offer_picture),
        ]);
    }

    /// Hands the picture in effect to the player now, or once the interval is over.
    fn offer_picture(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if let Some(picture) = self.tracks.picture.throttle.offer(picture(cx), now) {
            self.send_picture(picture, cx);
        } else if self.tracks.picture.flush.is_none() {
            let wait = self.tracks.picture.throttle.wait(now);
            self.tracks.picture.flush = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(wait).await;
                // The window may be gone; nothing is left to adjust then.
                let _ = this.update(cx, Self::flush_picture);
            }));
        }
    }

    /// Sends the change held back while the interval ran.
    fn flush_picture(&mut self, cx: &mut Context<Self>) {
        self.tracks.picture.flush = None;
        if let Some(picture) = self.tracks.picture.throttle.flush() {
            self.send_picture(picture, cx);
        }
    }

    fn send_picture(&self, picture: Option<Picture>, cx: &mut Context<Self>) {
        if let Some((player, _)) = &self.player {
            player.update(cx, |player, _| player.set_picture(picture));
        }
    }
}

/// The picture in effect, or `None` to show it as the media has it.
fn picture(cx: &App) -> Option<Picture> {
    effects::effective(cx).prefs.video.picture.applied()
}
