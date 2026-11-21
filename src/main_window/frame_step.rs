//! Frame stepping (S2.10.3): `.` shows the next frame and `,` the previous one, pausing
//! playback. Stepping back means seeking to the keyframe before and decoding forward, which is
//! slow on long GOPs, so a step that takes longer than [`SLOW_STEP`] shows a spinner over the
//! video until it's done (`osd.rs`). A held key queues steps up to a limit (the engine's).

use std::time::Duration;

use gpui::{App, Context, Window};

use super::MainWindow;
use crate::engine::values::StepDirection;

/// How long a step may take before the spinner shows.
const SLOW_STEP: Duration = Duration::from_millis(200);

/// Whether the running steps are taking a while.
#[derive(Default)]
pub struct FrameStepWait {
    /// Counts runs of steps, so only the newest run's timer marks it slow.
    run: u64,
    slow: bool,
}

impl MainWindow {
    /// Steps one frame `direction`, pausing first.
    pub(super) fn step_frame(
        &mut self,
        direction: StepDirection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(player) = self.loaded_player(cx) else { return };
        let was_stepping = player.read(cx).is_stepping();
        player.update(cx, |player, cx| player.frame_step(direction, cx));
        if !was_stepping && player.read(cx).is_stepping() {
            self.time_frame_steps(cx);
        }
    }

    /// Marks the run of steps that just started slow if it's still going after [`SLOW_STEP`].
    fn time_frame_steps(&mut self, cx: &mut Context<Self>) {
        self.frame_step.run += 1;
        self.frame_step.slow = false;
        let run = self.frame_step.run;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SLOW_STEP).await;
            // The window may be gone by then; there is nothing left to show.
            let _ = this.update(cx, |this, cx| this.mark_slow(run, cx));
        })
        .detach();
    }

    /// Marks the steps slow, if `run` is still the newest run.
    fn mark_slow(&mut self, run: u64, cx: &mut Context<Self>) {
        if self.frame_step.run == run {
            self.frame_step.slow = true;
            cx.notify();
        }
    }

    /// Whether a frame step has been running longer than [`SLOW_STEP`].
    pub(super) fn is_frame_step_slow(&self, cx: &App) -> bool {
        self.frame_step.slow
            && self.loaded_player(cx).is_some_and(|player| player.read(cx).is_stepping())
    }
}
