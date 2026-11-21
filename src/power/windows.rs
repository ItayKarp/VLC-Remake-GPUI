//! Windows: `SetThreadExecutionState`, which holds for the calling thread. The request is made
//! and dropped on the UI thread, which lives as long as the app.

use std::marker::PhantomData;

use anyhow::{Result, ensure};

const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
const ES_CONTINUOUS: u32 = 0x8000_0000;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn SetThreadExecutionState(flags: u32) -> u32;
}

/// The display and system kept on for this thread, until dropped.
pub struct Request {
    /// The state belongs to the thread that set it, so the request must not move.
    _thread_bound: PhantomData<*const ()>,
}

impl Request {
    /// Keeps the display and the system on.
    pub fn new(_app: &str, _reason: &str) -> Result<Self> {
        // SAFETY: a plain Win32 call with valid flags.
        let previous = unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED)
        };
        ensure!(previous != 0, "SetThreadExecutionState failed");
        Ok(Self { _thread_bound: PhantomData })
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        // SAFETY: a plain Win32 call; `ES_CONTINUOUS` alone clears the requirements.
        unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
    }
}
