//! Where the libVLC runtime lives (S1.1.4): the libraries and, beside them, `plugins/`.
//!
//! libVLC 3 reads its plugin folder from `VLC_PLUGIN_PATH` (it fails `libvlc_new` without it for
//! these layouts), and libVLC has no option for it. Setting an environment variable is only
//! sound while no other thread runs, so [`prepare_environment`] is called first thing in `main`.

use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::api::LIBRARIES;

/// Points at an unpacked VLC folder (holding `lib/` on macOS, the DLLs on Windows, and
/// `plugins/`) to use instead of the bundled one.
pub const RUNTIME_ENV: &str = "RVLC_LIBVLC_DIR";
/// Where libVLC 3 looks for its plugins.
pub const PLUGIN_PATH_ENV: &str = "VLC_PLUGIN_PATH";

/// Where to load libVLC from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    /// Folder holding `libvlc` and `libvlccore`, or `None` for the system search path.
    pub lib_dir: Option<PathBuf>,
    /// Folder holding the plugins, or `None` to leave libVLC's own default.
    pub plugin_dir: Option<PathBuf>,
}

impl Runtime {
    /// The libVLC libraries of an unpacked VLC under `root`, per OS.
    pub fn under(root: &Path) -> Self {
        let lib_dir = if cfg!(target_os = "macos") { root.join("lib") } else { root.to_owned() };
        Self { lib_dir: Some(lib_dir), plugin_dir: Some(root.join("plugins")) }
    }

    /// The runtime this process uses, located once.
    pub fn located() -> &'static Self {
        static LOCATED: OnceLock<Runtime> = OnceLock::new();
        LOCATED.get_or_init(Self::locate)
    }

    /// The runtime to use: [`RUNTIME_ENV`], else the one shipped with the app, else (in debug
    /// builds) the repository's `vendor/libvlc`, else the system's.
    fn locate() -> Self {
        if let Some(root) = env::var_os(RUNTIME_ENV) {
            return Self::under(Path::new(&root));
        }
        let bundled = env::current_exe().ok().and_then(|exe| bundled_root(&exe));
        let vendored = cfg!(debug_assertions).then(vendored_root);
        bundled
            .into_iter()
            .chain(vendored)
            .map(|root| Self::under(&root))
            .find(Self::exists)
            .unwrap_or(Self { lib_dir: None, plugin_dir: None })
    }

    fn exists(&self) -> bool {
        self.lib_dir.as_ref().is_some_and(|dir| dir.join(LIBRARIES[0]).is_file())
    }
}

/// Points libVLC at the located runtime's plugins, unless `VLC_PLUGIN_PATH` is already set.
///
/// # Safety
///
/// No other thread may read or write the environment during the call: call it at the start of
/// `main`, before any thread starts.
pub unsafe fn prepare_environment() {
    if env::var_os(PLUGIN_PATH_ENV).is_some() {
        return;
    }
    if let Some(plugins) = &Runtime::located().plugin_dir {
        // SAFETY: the caller promises no other thread touches the environment now.
        unsafe { env::set_var(PLUGIN_PATH_ENV, plugins) };
    }
}

/// Where the packaged app keeps libVLC, relative to its executable: `Contents/Frameworks/libvlc`
/// in a macOS bundle, the executable's own folder elsewhere.
fn bundled_root(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    if cfg!(target_os = "macos") {
        return Some(dir.parent()?.join("Frameworks").join("libvlc"));
    }
    Some(dir.to_owned())
}

/// The runtime unpacked into the repository for development, such as
/// `vendor/libvlc/macos-arm64`.
fn vendored_root() -> PathBuf {
    let arch = match env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        other => other,
    };
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor/libvlc")
        .join(format!("{}-{arch}", env::consts::OS))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_macos_layout_keeps_libraries_in_lib() {
        let runtime = Runtime::under(Path::new("/vlc"));
        let expected = if cfg!(target_os = "macos") { "/vlc/lib" } else { "/vlc" };
        assert_eq!(runtime.lib_dir, Some(PathBuf::from(expected)));
    }

    #[test]
    fn a_macos_bundle_keeps_libvlc_in_frameworks() {
        let exe = Path::new("/Applications/VLC.app/Contents/MacOS/RedesignedVLC");
        let root = bundled_root(exe).unwrap();
        if cfg!(target_os = "macos") {
            assert_eq!(root, Path::new("/Applications/VLC.app/Contents/Frameworks/libvlc"));
        } else {
            assert_eq!(root, Path::new("/Applications/VLC.app/Contents/MacOS"));
        }
    }
}
