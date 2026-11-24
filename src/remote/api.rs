//! What the web remote's page and the app say to each other: the commands a phone sends, and
//! the playback state and queue the app publishes for it to show.

use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};

/// The most queue items the page lists; a longer queue says how many more there are.
pub const MAX_QUEUE_ITEMS: usize = 500;

/// What a phone asks the app to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum RemoteCommand {
    /// Plays or pauses, as the Play/Pause button does.
    PlayPause,
    /// The Previous button: back to the item's start, or the item before.
    Previous,
    /// The Next button.
    Next,
    /// Sets the volume, in percent; the player keeps it under its maximum.
    Volume {
        /// The level.
        percent: u32,
    },
    /// Mutes or unmutes.
    ToggleMute,
    /// Plays the queued item with this id.
    Play {
        /// [`QueueItem::id`].
        id: u64,
    },
}

/// Where playback stands, as the page shows it.
#[allow(
    clippy::struct_excessive_bools,
    reason = "a JSON message: each flag is a field the page reads, not states of one machine"
)]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RemoteState {
    /// What plays, or the app's name with nothing loaded.
    pub title: String,
    /// Whether Play/Pause would pause.
    pub playing: bool,
    /// The position, in ms.
    pub position_ms: u64,
    /// The length, in ms; `None` for a live stream or before it is known.
    pub duration_ms: Option<u64>,
    /// The volume, in percent.
    pub volume: u32,
    /// The loudest the volume goes, in percent.
    pub max_volume: u32,
    /// Whether the sound is muted.
    pub muted: bool,
    /// Whether Previous would do anything.
    pub can_go_back: bool,
    /// Whether Next would do anything.
    pub can_go_next: bool,
    /// Changes whenever the queue does, so the page fetches it again only then.
    pub queue_revision: u64,
}

/// The queue, as the page lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct QueueList {
    /// The first [`MAX_QUEUE_ITEMS`] items.
    pub items: Vec<QueueItem>,
    /// How many items follow those listed.
    pub more: usize,
}

/// One queued item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct QueueItem {
    /// Names the item for [`RemoteCommand::Play`], wherever it moves.
    pub id: u64,
    /// Its name.
    pub title: String,
    /// Whether it is the current item.
    pub current: bool,
}

/// What the app last published, which the server hands to phones. The app updates it; the
/// server's threads read it.
#[derive(Clone, Debug, Default)]
pub struct Published(Arc<Mutex<(RemoteState, QueueList)>>);

impl Published {
    /// Replaces the state.
    pub fn set_state(&self, state: RemoteState) {
        self.lock().0 = state;
    }

    /// Replaces the queue.
    pub fn set_queue(&self, queue: QueueList) {
        self.lock().1 = queue;
    }

    /// The state, as JSON.
    pub fn state_json(&self) -> String {
        serde_json::to_string(&self.lock().0).unwrap_or_default()
    }

    /// The queue, as JSON.
    pub fn queue_json(&self) -> String {
        serde_json::to_string(&self.lock().1).unwrap_or_default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, (RemoteState, QueueList)> {
        // Nothing that holds the lock can panic midway, so a poisoned value is still whole.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_read_from_json() {
        let read = |text| serde_json::from_str::<RemoteCommand>(text).ok();
        assert_eq!(read(r#"{"command":"play_pause"}"#), Some(RemoteCommand::PlayPause));
        assert_eq!(
            read(r#"{"command":"volume","percent":80}"#),
            Some(RemoteCommand::Volume { percent: 80 })
        );
        assert_eq!(read(r#"{"command":"play","id":7}"#), Some(RemoteCommand::Play { id: 7 }));
        assert_eq!(read(r#"{"command":"eject"}"#), None);
        assert_eq!(read(r#"{"command":"volume","percent":-1}"#), None);
    }

    #[test]
    fn published_state_reads_as_json() {
        let published = Published::default();
        published.set_state(RemoteState {
            title: "Film".into(),
            volume: 80,
            ..RemoteState::default()
        });
        let json: serde_json::Value = serde_json::from_str(&published.state_json()).unwrap();
        assert_eq!(json["title"], "Film");
        assert_eq!(json["volume"], 80);
        assert_eq!(json["duration_ms"], serde_json::Value::Null);
    }
}
