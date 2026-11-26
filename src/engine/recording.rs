//! Recording the media playing (S8.6): what the backend reports about it.
//!
//! libVLC 3 records by copying the stream as it arrives into a new file, without re-encoding:
//! MPEG-TS sources natively, others remuxed by its `record` stream output, which picks the
//! container. It names the file itself (`vlc-record-<date>-<time>-<title>.<ext>`), so the UI
//! finds it in the folder once the recording ends.

/// What became of recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recording {
    /// It started; the file appears once enough of the stream has arrived to pick a container.
    Started,
    /// It ended: stopped, or the media ended, failed, stopped or was replaced. The file is
    /// finished.
    Stopped,
    /// Nothing is playing that can be recorded.
    Refused,
}
