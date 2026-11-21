//! The seek keys (S2.3.4): the arrows jump by the skip amounts of Settings → Controls (S5.6:
//! Shift very short, alone short, Alt medium, Ctrl or Cmd long), and Home and End go to the
//! start and the end. The player clamps jumps to the media and ignores them where it can't
//! seek. Each jump says how far it went and where it landed over the video (S2.10.2), and T says
//! where playback is.

use std::time::Duration;

use gpui::{App, Context, Entity, Window};

use super::MainWindow;
use super::osd::messages;
use crate::engine::player::Player;
use crate::engine::values::{Offset, SeekTarget};
use crate::settings::{self, SkipLength};

/// Which way a skip goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    /// Towards the start.
    Back,
    /// Towards the end.
    Forward,
}

/// An end of the media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Its first moment.
    Start,
    /// Its end: playback ends there.
    End,
}

impl MainWindow {
    /// Jumps a `length` skip `way` from the shown position.
    pub(super) fn skip(&mut self, way: Way, length: SkipLength, cx: &mut Context<Self>) {
        let Some(player) = self.seekable_player(cx) else { return };
        let jump = settings::get(cx).prefs.controls.skip(length);
        let millis = i64::try_from(jump.as_millis()).unwrap_or(i64::MAX);
        let offset = Offset::from_millis(if way == Way::Back { -millis } else { millis });
        player.update(cx, |player, cx| player.seek(SeekTarget::Relative(offset), cx));
        let landed = position_text(player.read(cx));
        self.show_osd(messages::skip(way, jump, &landed), cx);
    }

    /// Goes to the start or the end; the end needs a known length.
    pub(super) fn seek_to(&mut self, edge: Edge, cx: &mut Context<Self>) {
        let Some(player) = self.seekable_player(cx) else { return };
        let target = match edge {
            Edge::Start => Duration::ZERO,
            Edge::End => {
                let Some(duration) = player.read(cx).duration() else { return };
                duration
            }
        };
        player.update(cx, |player, cx| player.seek(SeekTarget::Absolute(target), cx));
        self.show_osd(position_text(player.read(cx)), cx);
    }

    /// T: says where playback is.
    pub(super) fn show_time(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        self.show_osd(position_text(player.read(cx)), cx);
    }

    /// The loaded player, unless its media is known not to seek (a live stream).
    fn seekable_player(&self, cx: &App) -> Option<Entity<Player>> {
        self.loaded_player(cx).filter(|player| player.read(cx).is_seekable() != Some(false))
    }

    /// Seeks the loaded player, if there is one.
    pub(super) fn seek_player(&self, target: SeekTarget, cx: &mut Context<Self>) {
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, |player, cx| player.seek(target, cx));
        }
    }
}

/// Where `player` is, as the messages over the video say it.
fn position_text(player: &Player) -> String {
    messages::position(player.position(), player.duration())
}
