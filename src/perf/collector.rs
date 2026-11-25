//! GPUI's frame timing (the `perf` feature): the frames a window drew and presented since the
//! last look.

use std::sync::Mutex;
use std::time::Instant;

use gpui::{FrameEvent, FrameTiming, FrameTimingCollector};

use super::frames::Frame;

/// The collector's place in GPUI's frame log, and a drawn frame waiting for its present.
struct State {
    collector: FrameTimingCollector,
    drawn: Option<FrameTiming>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// The frames the window with id `window` drew since the last call, each with when its draw
/// started. The first call turns GPUI's frame timing on and returns nothing.
pub fn take(window: u64) -> Vec<(Frame, Instant)> {
    let mut state = STATE.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(state) = state.as_mut() else {
        gpui::set_trace_enabled(true);
        *state = Some(State { collector: FrameTimingCollector::new(), drawn: None });
        return Vec::new();
    };
    let events = state.collector.collect_unseen();
    events.into_iter().filter_map(|event| pair(&mut state.drawn, event, window)).collect()
}

/// Pairs a draw of `window` with the present that follows it; a draw without one (the window
/// was hidden) counts with no present time.
fn pair(
    drawn: &mut Option<FrameTiming>,
    event: FrameEvent,
    window: u64,
) -> Option<(Frame, Instant)> {
    match event {
        FrameEvent::Draw(timing) if timing.window_id.as_u64() == window => {
            let earlier = drawn.replace(timing);
            earlier.map(|draw| frame(draw, std::time::Duration::ZERO))
        }
        FrameEvent::Present(present) if present.window_id.as_u64() == window => {
            let draw = drawn.take()?;
            Some(frame(draw, present.present_duration()))
        }
        FrameEvent::Draw(_) | FrameEvent::Present(_) => None,
    }
}

fn frame(draw: FrameTiming, present: std::time::Duration) -> (Frame, Instant) {
    let frame = Frame {
        draw: draw.draw_end.duration_since(draw.draw_start),
        present,
        refresh: draw.refresh_interval,
    };
    (frame, draw.draw_start)
}
