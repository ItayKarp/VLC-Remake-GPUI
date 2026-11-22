//! State that Settings rows show and edit but that belongs to something else, mirrored here so
//! a row's binding can reach it. None of it is written to the settings files: its owner keeps
//! it (the queue's Repeat and Shuffle live in the session file, S3.4).
//!
//! The owner copies its state in whenever it changes, and applies what a row writes back.

use crate::queue::Repeat;

/// The mirrored state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LiveState {
    /// The queue's Repeat (S3.3.4).
    pub repeat: Repeat,
    /// Whether the queue is shuffled (S3.3.4).
    pub shuffle: bool,
}
