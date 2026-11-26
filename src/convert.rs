//! Convert (S8.10): converting media to another format in the background.
//!
//! - [`Preset`]: the formats, and the libVLC options making each.
//! - [`output`]: where each conversion writes, never over a file that exists or its source.
//! - [`Queue`]: what was asked for and how it is going, a few at a time.
//! - [`Conversions`]: the app's one queue, its conversions running on threads of their own
//!   (`engine::convert`) while playback goes on, and the notification when a batch is over.

mod conversions;
mod output;
mod preset;
mod queue;

pub use conversions::{CONVERSIONS_TAG, Conversions, init};
pub use output::{Destination, is_source};
pub use preset::Preset;
pub use queue::{Job, Status};
