//! Held keys (S0.5.6). The OS repeats a held key, and GPUI dispatches the bound
//! action again for each repeat without saying it is one. The gate remembers which
//! commands ran since the last key release: a [`Repeat::Once`] command is ignored
//! until then, and a [`Repeat::Throttled`] one runs at most once per interval.

use std::collections::HashMap;
use std::time::Instant;

use super::commands::{Command, Repeat};

/// Tracks commands whose key may still be held. Release it on every key-up and
/// when the window loses focus (the key-up then goes elsewhere).
#[derive(Debug, Default)]
pub struct RepeatGate {
    /// When each held command last ran, by label.
    held: HashMap<&'static str, Instant>,
}

impl RepeatGate {
    /// Whether `command` should run for a key press at `now`; records it if so.
    pub fn admit(&mut self, command: &Command, now: Instant) -> bool {
        let Some(&last) = self.held.get(command.label) else {
            self.held.insert(command.label, now);
            return true;
        };
        match command.repeat {
            Repeat::Throttled(interval) if now.saturating_duration_since(last) >= interval => {
                self.held.insert(command.label, now);
                true
            }
            Repeat::Once | Repeat::Throttled(_) => false,
        }
    }

    /// A key was released: the next press of any command is a fresh one.
    pub fn release_all(&mut self) {
        self.held.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::keymap::actions::{PlayPause, SeekForward};
    use crate::keymap::commands::{HELD_KEY_INTERVAL, find};

    fn command(action: &dyn gpui::Action) -> &'static Command {
        find(action).expect("the action has a command")
    }

    #[test]
    fn holding_space_toggles_once() {
        let mut gate = RepeatGate::default();
        let start = Instant::now();
        let play = command(&PlayPause);
        assert!(gate.admit(play, start));
        assert!(!gate.admit(play, start + Duration::from_millis(500)));
        assert!(!gate.admit(play, start + Duration::from_secs(5)));
        gate.release_all();
        assert!(gate.admit(play, start + Duration::from_secs(6)));
    }

    #[test]
    fn holding_an_arrow_seeks_at_the_throttled_rate() {
        let mut gate = RepeatGate::default();
        let start = Instant::now();
        let seek = command(&SeekForward);
        let repeats = (0..20).map(|i| start + Duration::from_millis(30 * i));
        let ran = repeats.filter(|&at| gate.admit(seek, at)).count();
        // 600 ms of repeats every 30 ms: the first press, then one per interval.
        let expected = 1 + (570 / HELD_KEY_INTERVAL.as_millis()) as usize;
        assert_eq!(ran, expected);
    }

    #[test]
    fn a_new_press_after_release_is_not_throttled() {
        let mut gate = RepeatGate::default();
        let start = Instant::now();
        let seek = command(&SeekForward);
        assert!(gate.admit(seek, start));
        gate.release_all();
        assert!(gate.admit(seek, start + Duration::from_millis(10)));
    }
}
