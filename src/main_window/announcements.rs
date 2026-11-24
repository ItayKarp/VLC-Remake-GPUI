//! What the main window tells a screen reader (S9.1.2): the play state and the item, when
//! playback starts, pauses, stops, finishes or fails, in the status line's words ("Playing
//! Movie.mkv", "Couldn't play Movie.mkv: the file is damaged."). Errors interrupt; the rest
//! waits its turn. Toasts speak for themselves (`ui::toast`).

use gpui::{AnyElement, Context, IntoElement};

use super::MainWindow;
use crate::engine::state::PlaybackState;
use crate::ui::announcer::Politeness;

impl MainWindow {
    /// Speaks the status line for `state`, if it is a state worth speaking.
    pub(super) fn announce_state(&mut self, state: PlaybackState, cx: &Context<Self>) {
        let Some(politeness) = politeness(state) else { return };
        let Some((text, _)) = self.status(cx) else { return };
        self.announcer.announce(text, politeness);
    }

    /// The toasts, and the invisible live region playback news is spoken from.
    pub(super) fn render_messages(&self) -> impl IntoIterator<Item = AnyElement> {
        let region = self.announcer.render("playback-announcer");
        [Some(self.toasts.clone().into_any_element()), region.map(IntoElement::into_any_element)]
            .into_iter()
            .flatten()
    }
}

/// How urgently `state` is spoken; `None` for states that pass in a moment (opening,
/// buffering), which would only talk over the state that follows.
fn politeness(state: PlaybackState) -> Option<Politeness> {
    match state {
        PlaybackState::Idle | PlaybackState::Opening | PlaybackState::Buffering => None,
        PlaybackState::Playing
        | PlaybackState::Paused
        | PlaybackState::Ended
        | PlaybackState::Stopped => Some(Politeness::Polite),
        PlaybackState::Error(_) => Some(Politeness::Assertive),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::state::ErrorKind;

    #[test]
    fn passing_states_are_not_spoken() {
        assert_eq!(politeness(PlaybackState::Opening), None);
        assert_eq!(politeness(PlaybackState::Buffering), None);
        assert_eq!(politeness(PlaybackState::Idle), None);
    }

    #[test]
    fn errors_interrupt_and_the_rest_waits() {
        assert_eq!(
            politeness(PlaybackState::Error(ErrorKind::Corrupt)),
            Some(Politeness::Assertive)
        );
        assert_eq!(politeness(PlaybackState::Paused), Some(Politeness::Polite));
        assert_eq!(politeness(PlaybackState::Playing), Some(Politeness::Polite));
    }
}
