//! A launcher that records what it is asked to start, for the updater's tests.

use std::cell::RefCell;
use std::io;
use std::process::Command;

use super::{Launcher, System};

/// Records each command instead of starting it; refuses them all when made with
/// [`Recorder::failing`].
#[derive(Default)]
pub struct Recorder {
    launched: RefCell<Vec<Vec<String>>>,
    fails: bool,
}

impl Recorder {
    /// A launcher whose programs never start.
    pub fn failing() -> Self {
        Self { fails: true, ..Self::default() }
    }

    /// Each command asked for: its program, then its arguments.
    pub fn launched(&self) -> Vec<Vec<String>> {
        self.launched.borrow().clone()
    }
}

impl Launcher for Recorder {
    fn launch(&self, command: Command) -> io::Result<()> {
        let words = std::iter::once(command.get_program()).chain(command.get_args());
        self.launched.borrow_mut().push(words.map(|word| word.to_string_lossy().into()).collect());
        if self.fails { Err(io::Error::from(io::ErrorKind::NotFound)) } else { Ok(()) }
    }
}

#[test]
fn a_missing_program_does_not_start() {
    let missing = Command::new("rvlc-test-no-such-program");
    assert_eq!(System.launch(missing).unwrap_err().kind(), io::ErrorKind::NotFound);
}
