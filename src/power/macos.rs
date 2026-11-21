//! macOS: an IOKit power assertion of type `PreventUserIdleDisplaySleep`.

use anyhow::{Result, ensure};
use objc2_core_foundation::CFString;

/// `kIOPMAssertionTypePreventUserIdleDisplaySleep`.
const PREVENT_DISPLAY_SLEEP: &str = "PreventUserIdleDisplaySleep";
/// `kIOPMAssertionLevelOn`.
const LEVEL_ON: u32 = 255;
/// `kIOReturnSuccess`.
const SUCCESS: i32 = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOPMAssertionCreateWithName(
        kind: &CFString,
        level: u32,
        name: &CFString,
        id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(id: u32) -> i32;
}

/// A held assertion, released on drop.
pub struct Request(u32);

impl Request {
    /// Creates the assertion, named for Activity Monitor and `pmset -g assertions`.
    pub fn new(app: &str, reason: &str) -> Result<Self> {
        let kind = CFString::from_str(PREVENT_DISPLAY_SLEEP);
        let name = CFString::from_str(&format!("{app}: {reason}"));
        let mut id = 0;
        // SAFETY: both strings are live CFStrings and `id` is a writable out pointer.
        let status = unsafe { IOPMAssertionCreateWithName(&kind, LEVEL_ON, &name, &raw mut id) };
        ensure!(status == SUCCESS, "IOPMAssertionCreateWithName failed: {status:#x}");
        Ok(Self(id))
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        // SAFETY: the id came from a successful create and is released once.
        unsafe { IOPMAssertionRelease(self.0) };
    }
}
