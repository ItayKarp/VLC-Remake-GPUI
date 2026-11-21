//! The seek keys (S2.3.4): the arrows jump by the skip amounts of Settings → Controls (S5.6:
//! Shift very short, alone short, Alt medium, Ctrl or Cmd long), and Home and End go to the
//! start and the end. The player clamps jumps to the media and ignores them where it can't
//! seek.

use std::time::Duration;

use gpui::Context;

use super::MainWindow;
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
        let jump = settings::get(cx).prefs.controls.skip(length);
        let millis = i64::try_from(jump.as_millis()).unwrap_or(i64::MAX);
        let offset = Offset::from_millis(if way == Way::Back { -millis } else { millis });
        self.seek_player(SeekTarget::Relative(offset), cx);
    }

    /// Goes to the start or the end; the end needs a known length.
    pub(super) fn seek_to(&mut self, edge: Edge, cx: &mut Context<Self>) {
        let target = match edge {
            Edge::Start => Duration::ZERO,
            Edge::End => {
                let duration = self.loaded_player(cx).and_then(|player| player.read(cx).duration());
                let Some(duration) = duration else { return };
                duration
            }
        };
        self.seek_player(SeekTarget::Absolute(target), cx);
    }

    fn seek_player(&self, target: SeekTarget, cx: &mut Context<Self>) {
        if let Some(player) = self.loaded_player(cx) {
            player.update(cx, |player, cx| player.seek(target, cx));
        }
    }
}
