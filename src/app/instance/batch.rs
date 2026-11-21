//! Folding launches that arrive together into one (S0.6.1 edge cases).
//!
//! Selecting five videos in Explorer and pressing Enter starts five processes at
//! once. The first becomes the instance and the other four forward one file each.
//! Taken one by one, each "replace the queue" would throw away the previous file, so
//! launches that arrive within [`BATCH_WINDOW`] of each other become one request,
//! and a batch that follows the previous delivery (or startup) that closely is added
//! to it instead of replacing it.

use std::time::{Duration, Instant};

use crate::app::request::{Launch, Placement, StartScreen};

/// Launches closer together than this belong to the same user action.
pub const BATCH_WINDOW: Duration = Duration::from_millis(300);

/// Remembers when the last batch was delivered.
#[derive(Clone, Copy, Debug)]
pub struct Batcher {
    last_delivery: Instant,
}

impl Batcher {
    /// Starts counting from `now`, when the app opened its first window: launches
    /// forwarded during startup join the files this process was started with.
    pub fn new(now: Instant) -> Self {
        Batcher { last_delivery: now }
    }

    /// Combines a batch (in arrival order) whose first launch arrived at
    /// `first_arrival`, delivered at `now`.
    pub fn combine(&mut self, batch: Vec<Launch>, first_arrival: Instant, now: Instant) -> Launch {
        let joins_previous =
            first_arrival.saturating_duration_since(self.last_delivery) <= BATCH_WINDOW;
        self.last_delivery = now;

        let all_enqueue = batch.iter().all(|launch| launch.open.placement == Placement::Enqueue);
        let mut combined = Launch::default();
        if joins_previous || all_enqueue {
            combined.open.placement = Placement::Enqueue;
        }
        for launch in batch {
            if launch.start != StartScreen::Auto {
                combined.start = launch.start;
            }
            combined.open.fullscreen |= launch.open.fullscreen;
            combined.open.items.extend(launch.open.items);
        }
        combined
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::app::media::Media;
    use crate::app::request::OpenRequest;

    fn opening(name: &str, placement: Placement) -> Launch {
        let items = vec![Media::File(PathBuf::from(name))];
        Launch {
            open: OpenRequest { items, placement, ..OpenRequest::default() },
            ..Launch::default()
        }
    }

    fn names(launch: &Launch) -> Vec<String> {
        launch.open.items.iter().map(Media::display_name).collect()
    }

    #[test]
    fn a_multi_file_selection_becomes_one_replace() {
        let start = Instant::now();
        let mut batcher = Batcher::new(start);
        let later = start + Duration::from_secs(10);
        let batch = vec![opening("a", Placement::Replace), opening("b", Placement::Replace)];
        let combined = batcher.combine(batch, later, later + BATCH_WINDOW);
        assert_eq!(names(&combined), ["a", "b"]);
        assert_eq!(combined.open.placement, Placement::Replace);
    }

    #[test]
    fn launches_during_startup_join_the_first_window() {
        let start = Instant::now();
        let mut batcher = Batcher::new(start);
        let batch = vec![opening("b", Placement::Replace)];
        let combined = batcher.combine(batch, start, start + BATCH_WINDOW);
        assert_eq!(combined.open.placement, Placement::Enqueue);
    }

    #[test]
    fn keeps_enqueue_reset_and_fullscreen() {
        let start = Instant::now();
        let mut batcher = Batcher::new(start);
        let later = start + Duration::from_secs(10);
        let mut reset = opening("b", Placement::Enqueue);
        reset.start = StartScreen::Onboarding;
        reset.open.fullscreen = true;
        let combined = batcher.combine(vec![opening("a", Placement::Enqueue), reset], later, later);
        assert_eq!(combined.open.placement, Placement::Enqueue);
        assert_eq!(combined.start, StartScreen::Onboarding);
        assert!(combined.open.fullscreen);
    }
}
