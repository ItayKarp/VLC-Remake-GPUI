//! The third-party licenses (S9.9.2): libVLC's notice with the LGPL and GPL texts, and every
//! crate the app is built from under its license. `tools/licenses/generate.sh` writes the page
//! from `Cargo.lock` with cargo-about; the app carries it and About VLC opens it in the browser.

use std::io;
use std::path::{Path, PathBuf};

/// The licenses page, as `tools/licenses/generate.sh` wrote it.
const PAGE: &str = include_str!("../assets/licenses/third-party.html");
/// The page's name where it is written out to be opened.
const FILE_NAME: &str = "third-party-licenses.html";

/// Writes the licenses page into `dir` (made if missing) and returns its path. A page already
/// there with the same text is left as it is.
pub fn write_page(dir: &Path) -> io::Result<PathBuf> {
    let path = dir.join(FILE_NAME);
    if std::fs::read_to_string(&path).is_ok_and(|text| text == PAGE) {
        return Ok(path);
    }
    std::fs::create_dir_all(dir)?;
    std::fs::write(&path, PAGE)?;
    Ok(path)
}

#[cfg(test)]
mod tests;
