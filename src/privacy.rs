//! Settings → Privacy's behaviour (S5.5): which media is never remembered, clearing what was,
//! and the OS keychain that holds the last.fm password.
//!
//! - [`exclusions`]: "Never remember items matching", over the patterns of [`glob`].
//! - [`clear`]: Clear History and "Clear history now", with its confirmation.
//! - [`keychain`]: secrets kept in the OS keychain rather than a settings file.

pub mod clear;
pub mod exclusions;
pub mod glob;
pub mod keychain;
