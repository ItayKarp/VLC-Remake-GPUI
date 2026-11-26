//! Recording the media playing (S8.6) through the input's own variables, as VLC's Record button
//! does: `input-record-path` says where, and setting `record` starts or stops it. libVLC 3's API
//! has no call for it, so these go through the core ([`super::core_vars`]).
//!
//! The input reports nothing when a recording ends with it. The worker ends it itself when the
//! media ends, fails, stops or is replaced; for anything else (a reopen for a video filter, a
//! recording libVLC gave up) it reads `record` back every [`RECORD_POLL`] while one runs: once
//! it is off, or the input is gone, the recording ended and its file is finished.

use std::ffi::{CStr, CString};
use std::path::Path;
use std::time::{Duration, Instant};

use super::core_vars::{INPUT, Var};
use super::worker::Worker;
use crate::engine::backend::BackendEvent;
use crate::engine::recording::Recording;

/// How often a running recording is checked on.
const RECORD_POLL: Duration = Duration::from_millis(500);
/// The input's variables: whether it is recording, where to, and whether it can.
const RECORD: &CStr = c"record";
const RECORD_PATH: &CStr = c"input-record-path";
const CAN_RECORD: &CStr = c"can-record";

impl Worker {
    /// Starts recording into `folder`, or stops with `None`.
    pub(super) fn record(&mut self, folder: Option<&Path>) {
        match folder {
            Some(folder) => self.start_recording(folder),
            None => self.stop_recording(),
        }
    }

    /// Starts recording the playing input into `folder`; refused without an input that can.
    fn start_recording(&mut self, folder: &Path) {
        if self.recording_check.is_some() {
            return;
        }
        let path = CString::new(folder.to_string_lossy().into_owned());
        let (true, Ok(path)) = (self.can_record(), path) else {
            self.shared.send(BackendEvent::Recording(Recording::Refused));
            return;
        };
        let player = self.handles.player;
        // SAFETY: the player is live. The path is set first: the input reads it as it starts.
        unsafe {
            self.vlc.set_child_var(player, INPUT, RECORD_PATH, Var::String(&path));
            self.vlc.set_child_var(player, INPUT, RECORD, Var::Bool(true));
        }
        tracing::info!("libvlc: recording into {}", crate::logging::path(folder));
        self.recording_check = Some(Instant::now() + RECORD_POLL);
        self.shared.send(BackendEvent::Recording(Recording::Started));
    }

    /// Whether an input is playing that says it can record.
    fn can_record(&self) -> bool {
        // SAFETY: the player is live.
        unsafe { self.vlc.child_bool(self.handles.player, INPUT, CAN_RECORD) == Some(true) }
    }

    /// Stops the recording, which finishes its file, and says so.
    fn stop_recording(&mut self) {
        if self.recording_check.take().is_none() {
            return;
        }
        // SAFETY: the player is live. An input that is gone took its recording with it.
        unsafe { self.vlc.set_child_var(self.handles.player, INPUT, RECORD, Var::Bool(false)) };
        self.shared.send(BackendEvent::Recording(Recording::Stopped));
    }

    /// Says a recording ended with the media: it ended or failed, or a stop or another open
    /// ends it. libVLC 3 keeps an ended input, with `record` still on, until the next stop.
    pub(super) fn recording_ends(&mut self) {
        if self.recording_check.take().is_some() {
            self.shared.send(BackendEvent::Recording(Recording::Stopped));
        }
    }

    /// How long until the running recording is checked on; `None` without one.
    pub(super) fn until_record_check(&self, now: Instant) -> Option<Duration> {
        self.recording_check.map(|due| due.saturating_duration_since(now))
    }

    /// Checks on the running recording if due: it ended once the input stopped recording or is
    /// gone.
    pub(super) fn check_recording_if_due(&mut self, now: Instant) {
        let Some(due) = self.recording_check else { return };
        if now < due {
            return;
        }
        // SAFETY: the player is live.
        let recording = unsafe { self.vlc.child_bool(self.handles.player, INPUT, RECORD) };
        if recording == Some(true) {
            self.recording_check = Some(now + RECORD_POLL);
        } else {
            tracing::info!("libvlc: the recording ended with the input");
            self.recording_ends();
        }
    }
}
