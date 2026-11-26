//! Checking a picked custom thumbnail (S3.5 edge cases): it must be a readable image file of a
//! sensible size. Its kind is read from the file's first bytes, not its name, so a renamed
//! document is refused. Copying it into the thumbnail cache, downscaled and as its first frame
//! only, comes with the thumbnail work (S3.5.6, S4.4).

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// The largest image file taken as a thumbnail, in bytes.
pub const MAX_BYTES: u64 = 25 * 1024 * 1024;
/// How many bytes are enough to tell an image format.
const HEADER_BYTES: u64 = 64;

/// Why a picked file can't be a thumbnail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// It couldn't be read.
    Unreadable,
    /// It isn't an image.
    NotAnImage,
    /// It's larger than [`MAX_BYTES`].
    TooLarge,
}

impl Refused {
    /// What the dialog says about it.
    pub fn message(self) -> &'static str {
        match self {
            Self::Unreadable => tr!("Couldn't read that image."),
            Self::NotAnImage => tr!("That file isn't an image."),
            Self::TooLarge => tr!("That image is too large. Pick one under 25 MB."),
        }
    }
}

/// Whether the file at `path` can be a thumbnail. Reads the disk, so run it off the UI thread.
pub fn check(path: &Path) -> Result<(), Refused> {
    let file = File::open(path).map_err(|_| Refused::Unreadable)?;
    let size = file.metadata().map_err(|_| Refused::Unreadable)?.len();
    if size > MAX_BYTES {
        return Err(Refused::TooLarge);
    }
    let mut header = Vec::new();
    file.take(HEADER_BYTES).read_to_end(&mut header).map_err(|_: io::Error| Refused::Unreadable)?;
    image::guess_format(&header).map(drop).map_err(|_| Refused::NotAnImage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opening::scratch::scratch_dir;

    /// The start of a PNG file.
    const PNG_HEADER: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";

    #[test]
    fn images_are_told_by_their_content() {
        let dir = scratch_dir();
        let image = dir.join("cover.dat");
        std::fs::write(&image, PNG_HEADER).expect("write");
        assert_eq!(check(&image), Ok(()));
        let text = dir.join("notes.png");
        std::fs::write(&text, "not an image").expect("write");
        assert_eq!(check(&text), Err(Refused::NotAnImage));
        assert_eq!(check(&dir.join("missing.png")), Err(Refused::Unreadable));
    }
}
