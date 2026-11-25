//! Starting other programs for the updater: the Windows Installer at quit, and the previous
//! version after a rollback (S9.5.2). Behind a trait, so tests can see what would start without
//! starting anything.

use std::io;
use std::process::Command;

/// Starts a program without waiting for it.
pub trait Launcher {
    /// Starts `command`; an error when it couldn't be started.
    fn launch(&self, command: Command) -> io::Result<()>;
}

/// Starts programs on this computer.
pub struct System;

impl Launcher for System {
    fn launch(&self, mut command: Command) -> io::Result<()> {
        command.spawn().map(drop)
    }
}

#[cfg(test)]
pub mod tests;
