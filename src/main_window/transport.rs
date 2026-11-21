//! The transport commands (S2.2.2, S2.2.3): Play/Pause, Stop, Previous and Next. Their keys,
//! the control bar and its overflow menu all run them through here.
//!
//! Play/Pause flips what the player was last asked for, not what the engine last reported, so
//! clicks faster than the engine answers each count, and the button (drawn from the same
//! request) never shows the opposite of what will happen.

mod rules;

use std::time::Duration;

use gpui::{App, Context, Entity, Window};

use self::rules::{Back, Ends, Queue};
use super::MainWindow;
use crate::engine::player::Player;
use crate::engine::state::PlaybackState;
use crate::engine::values::SeekTarget;

impl MainWindow {
    /// Plays or pauses what's loaded. With nothing to play it asks for files: an empty list has
    /// nothing else Play could mean.
    pub(super) fn play_pause(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else {
            if self.jobs.is_empty() {
                Self::prompt_for_files(window, cx);
            }
            return;
        };
        // A failed item has nothing to resume: Play tries it again.
        if matches!(player.read(cx).state(), PlaybackState::Error(_)) {
            if let Some(current) = self.current {
                self.play(current, window, cx);
            }
            return;
        }
        player.update(cx, Player::toggle);
    }

    /// Stops playback; Play starts the item again.
    pub(super) fn stop(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, Player::stop);
        }
    }

    /// Restarts the current item past its first seconds, else plays the one before it.
    pub(super) fn previous(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.queue().back(self.position(cx)) {
            Some(Back::Restart) => self.restart(cx),
            Some(Back::Item(index)) => self.play(index, window, cx),
            None => {}
        }
    }

    /// Plays the next item.
    pub(super) fn next(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.queue().next() {
            self.play(index, window, cx);
        }
    }

    /// Whether Previous would do anything now.
    pub(super) fn can_go_back(&self, cx: &App) -> bool {
        self.queue().back(self.position(cx)).is_some()
    }

    /// Whether Next would do anything now.
    pub(super) fn can_go_next(&self) -> bool {
        self.queue().next().is_some()
    }

    /// The play list as Previous and Next see it. Until the queue owns Repeat (S3.1), the list
    /// stops at its ends.
    fn queue(&self) -> Queue {
        Queue { current: self.current, len: self.files.len(), ends: Ends::Stop }
    }

    /// The player, while it has media (in any state, ended and failed included).
    pub(super) fn loaded_player(&self, cx: &App) -> Option<Entity<Player>> {
        let (player, _) = self.player.as_ref()?;
        player.read(cx).media().is_some().then(|| player.clone())
    }

    fn position(&self, cx: &App) -> Duration {
        self.loaded_player(cx).map_or(Duration::ZERO, |player| player.read(cx).position())
    }

    /// Plays the current item from its start: a seek while it's loaded, else a fresh open.
    fn restart(&self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        player.update(cx, |player, cx| {
            if player.state().is_active() {
                player.seek(SeekTarget::Absolute(Duration::ZERO), cx);
            } else {
                player.play(cx);
            }
        });
    }
}
