//! Which process is the running instance, and where it listens (S0.6.1).
//!
//! An OS file lock on `instance.lock`, next to the local settings, decides it. The
//! OS drops the lock when the process dies, so a crash never leaves a stale lock.
//! Since the lock file lives in the user's own settings folder, the lock is per
//! user (and per `RVLC_CONFIG_DIR`, so portable copies and tests don't collide).
//! On Windows the session ID is part of the name too, so fast user switching and
//! Remote Desktop sessions each get their own instance.
//!
//! The listener's name is derived from the lock file's path: a named pipe on
//! Windows, and a Unix domain socket in the per-user runtime folder elsewhere
//! (Linux's abstract socket names are shared by every user, so they aren't used).

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use interprocess::local_socket::Name;
use interprocess::local_socket::prelude::*;

use crate::settings::Locations;

/// The socket file's mode on Unix: only its owner may connect.
#[cfg(unix)]
const SOCKET_MODE: u32 = 0o600;

/// Where one user's instance keeps its lock and listener.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstancePaths {
    /// The file whose OS lock marks the running instance.
    pub lock_file: PathBuf,
    /// The listener's name: a pipe name on Windows, a socket path elsewhere.
    socket: PathBuf,
}

impl InstancePaths {
    /// The paths for the settings this process uses, or `None` when settings have
    /// no folder (then there's nowhere to put the lock, and the app runs alone).
    pub fn resolve() -> Option<Self> {
        let locations = Locations::resolve()?;
        Some(Self::in_dir(locations.local.parent()?))
    }

    /// The paths for a lock file in `dir`.
    pub fn in_dir(dir: &Path) -> Self {
        let lock_file = dir.join(format!("instance{}.lock", session_suffix()));
        let key = fnv1a(lock_file.as_os_str().as_encoded_bytes());
        InstancePaths { socket: socket_path(key), lock_file }
    }

    /// The listener's name, for binding and connecting.
    pub fn name(&self) -> io::Result<Name<'_>> {
        #[cfg(windows)]
        {
            self.socket.as_os_str().to_ns_name::<interprocess::local_socket::GenericNamespaced>()
        }
        #[cfg(not(windows))]
        {
            self.socket.as_path().to_fs_name::<interprocess::local_socket::GenericFilePath>()
        }
    }

    /// Lets only this user connect to the socket (S9.7.4): a forwarded launch opens whatever it
    /// names, so another account must not be able to send one, even when the socket sits in a
    /// shared temp folder. Windows keeps pipes apart per user and session by name and by the
    /// pipe's default security.
    #[cfg_attr(not(unix), allow(clippy::unused_self, reason = "pipes have no file mode"))]
    pub fn restrict_socket(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.socket, fs::Permissions::from_mode(SOCKET_MODE))?;
        }
        Ok(())
    }

    /// Deletes a socket file left behind by a crashed instance. Only call this while
    /// holding the lock, so it can't be a live instance's socket.
    #[cfg_attr(not(unix), allow(clippy::unused_self, reason = "named pipes leave nothing behind"))]
    pub fn remove_stale_socket(&self) {
        #[cfg(unix)]
        match fs::remove_file(&self.socket) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => {
                tracing::warn!("instance: couldn't remove {}: {err}", self.socket.display());
            }
            _ => {}
        }
    }

    /// Takes the instance lock. `Ok(None)` means another process holds it.
    pub fn try_lock(&self) -> io::Result<Option<File>> {
        if let Some(dir) = self.lock_file.parent() {
            fs::create_dir_all(dir)?;
        }
        let file =
            OpenOptions::new().create(true).write(true).truncate(false).open(&self.lock_file)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(err)) => Err(err),
        }
    }
}

/// A 64-bit FNV-1a hash. Unlike `DefaultHasher`, it never changes between Rust
/// versions, so two builds of the app agree on the listener's name.
fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    bytes.iter().fold(OFFSET, |hash, &byte| (hash ^ u64::from(byte)).wrapping_mul(PRIME))
}

/// The pipe name on Windows: pipes live in one machine-wide namespace, so the key
/// (from the per-user lock path and the session) keeps users apart.
#[cfg(windows)]
fn socket_path(key: u64) -> PathBuf {
    PathBuf::from(format!("RedesignedVLC-{key:016x}"))
}

/// The socket file: in `$XDG_RUNTIME_DIR` (per user and login, mode 0700) when
/// there is one, else the temp folder (per user on macOS). Kept short because
/// socket paths are limited to about 100 bytes.
#[cfg(not(windows))]
fn socket_path(key: u64) -> PathBuf {
    let dir = dirs::runtime_dir().unwrap_or_else(std::env::temp_dir);
    dir.join(format!("rvlc-{key:016x}.sock"))
}

/// `-<session id>` on Windows, so each logon session has its own instance.
#[cfg(windows)]
fn session_suffix() -> String {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ProcessIdToSessionId(process_id: u32, session_id: *mut u32) -> i32;
    }
    let mut session = 0;
    // SAFETY: `session` is a valid, writable u32 for the duration of the call.
    let ok = unsafe { ProcessIdToSessionId(std::process::id(), &raw mut session) };
    if ok == 0 { String::new() } else { format!("-{session}") }
}

/// Empty elsewhere: the lock file is already in the user's own folder, and
/// `$XDG_RUNTIME_DIR` is per login.
#[cfg(not(windows))]
fn session_suffix() -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rvlc-lock-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("temp dir is writable");
        dir
    }

    #[test]
    fn only_one_holder_at_a_time() {
        let dir = scratch_dir("holder");
        let paths = InstancePaths::in_dir(&dir);
        let first = paths.try_lock().expect("lock file opens").expect("nobody holds it yet");
        assert!(paths.try_lock().expect("lock file opens").is_none());
        drop(first);
        assert!(paths.try_lock().expect("lock file opens").is_some());
        fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn names_differ_per_folder_and_are_stable() {
        let a = InstancePaths::in_dir(Path::new("one"));
        let b = InstancePaths::in_dir(Path::new("two"));
        assert_ne!(a.socket, b.socket);
        assert_eq!(a, InstancePaths::in_dir(Path::new("one")));
        assert!(a.name().is_ok());
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[cfg(unix)]
    #[test]
    fn only_the_user_may_connect() {
        use std::os::unix::fs::PermissionsExt;
        let paths = InstancePaths::in_dir(&scratch_dir("mode"));
        paths.remove_stale_socket();
        let _listener = std::os::unix::net::UnixListener::bind(&paths.socket).expect("a socket");
        paths.restrict_socket().expect("the mode changes");
        let mode = fs::metadata(&paths.socket).expect("the socket").permissions().mode();
        assert_eq!(mode & 0o777, SOCKET_MODE);
        paths.remove_stale_socket();
    }
}
