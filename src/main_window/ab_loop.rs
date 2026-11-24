//! The A-B loop (S8.13.4, not designed): Shift+L, VLC's key, and the macOS Playback menu's A-B
//! Loop mark A, then B, where playback is, then end the loop, each saying so over the video.

use gpui::Context;

use super::MainWindow;
use super::osd::messages;
use crate::engine::player::Player;

impl MainWindow {
    /// Marks the loop's next point, or ends it; media that can't seek says it can't loop.
    pub(super) fn loop_a_to_b(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let marks = player.update(cx, Player::mark_ab_loop);
        self.show_osd(messages::ab_loop(marks), cx);
    }
}
