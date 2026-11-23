//! The main window's side of the picture's adjustments (S7.1, S7.3) and color filters (S7.4): it
//! gives the player the ones in effect, Settings → Video's with the settings panel's session
//! values over them, and follows both. While a slider is dragged the player gets at most about 30 changes a second (S7.1.6),
//! the last one always arriving, so the video doesn't stutter under a flood of filter changes.

use std::time::{Duration, Instant};

use gpui::{App, Context, Entity, Subscription, Task};

use super::MainWindow;
use crate::effects::{self, EffectSession};
use crate::engine::colors::Colors;
use crate::engine::media_options::Picture;
use crate::engine::player::Player;
use crate::settings::SettingsStore;
use crate::ui::slider::Throttle;

/// The shortest time between two changes the player is given: about 30 a second.
const APPLY_INTERVAL: Duration = Duration::from_millis(33);

/// How the picture is shown: its adjustments and enhancements, and its color filters.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Look {
    picture: Option<Picture>,
    colors: Colors,
}

/// What the window keeps to apply the picture.
pub(super) struct PictureWatch {
    /// Paces the changes.
    throttle: Throttle<Look>,
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
        let look = look(cx);
        player.update(cx, |player, _| look.apply(player));
        self.tracks.picture = PictureWatch::following([
            cx.observe_global::<SettingsStore>(Self::offer_picture),
            cx.observe_global::<EffectSession>(Self::offer_picture),
        ]);
    }

    /// Hands the look in effect to the player now, or once the interval is over.
    fn offer_picture(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        if let Some(look) = self.tracks.picture.throttle.offer(look(cx), now) {
            self.send_look(look, cx);
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
        if let Some(look) = self.tracks.picture.throttle.flush() {
            self.send_look(look, cx);
        }
    }

    fn send_look(&self, look: Look, cx: &mut Context<Self>) {
        if let Some((player, _)) = &self.player {
            player.update(cx, |player, _| look.apply(player));
        }
    }
}

impl Look {
    /// Gives `player` the look; it passes on only what changed.
    fn apply(self, player: &mut Player) {
        player.set_picture(self.picture);
        player.set_colors(self.colors);
    }
}

/// The look in effect: the picture's adjustments (`None` to show it as the media has it) and the
/// color filters.
fn look(cx: &App) -> Look {
    let video = effects::effective(cx).prefs.video;
    Look { picture: video.picture.applied(), colors: video.colors.applied() }
}
