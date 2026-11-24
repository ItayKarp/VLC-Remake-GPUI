//! The [`Player`]'s statistics (S8.13.2): read from the backend when the UI asks, and kept
//! until the next open.

use super::Player;
use crate::engine::statistics::Statistics;

impl Player {
    /// What the backend last counted about the open media; `None` until read once.
    pub fn statistics(&self) -> Option<&Statistics> {
        self.info.statistics.as_ref()
    }

    /// Asks the backend for its counts; they arrive in [`Self::statistics`]. Nothing while no
    /// media is open.
    pub fn read_statistics(&mut self) {
        if self.state.is_active() {
            self.with_backend(|backend| backend.read_statistics());
        }
    }
}
