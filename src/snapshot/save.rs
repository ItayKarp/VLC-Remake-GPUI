//! Writing a snapshot: encoding the picture, finding a folder that takes it and a name that
//! isn't taken, and the thumbnail its toast shows.

use std::fs::{self, OpenOptions};
use std::io::{self, Cursor, Write as _};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbaImage};

use super::{SnapshotFormat, folder, naming};

/// The thumbnail's size at most, in pixels: the toast draws it at half that on a HiDPI screen.
const THUMBNAIL: (u32, u32) = (192, 108);
/// JPEG's quality, out of 100: hard to tell from the picture, a fraction of PNG's size.
const JPEG_QUALITY: u8 = 92;
/// How many numbered names to try before giving up on a folder.
const MAX_ATTEMPTS: u32 = 1000;

/// What to call a snapshot and where and how to save it.
pub struct Request {
    /// The media's title, which names the file.
    pub title: String,
    /// Where playback was, which names the file too.
    pub position: Duration,
    /// What to save it as.
    pub format: SnapshotFormat,
    /// "Save snapshots to", as typed.
    pub folder: String,
}

/// A saved snapshot.
pub struct Saved {
    /// The file.
    pub path: PathBuf,
    /// The folder asked for, when it couldn't be written and the file went elsewhere.
    pub instead_of: Option<PathBuf>,
    /// A small copy of the picture, in BGRA for GPUI.
    pub thumbnail: RgbaImage,
}

/// Saves `picture` as `request` says, in the first folder that takes it.
pub fn save(picture: &RgbaImage, request: &Request) -> Result<Saved> {
    save_in(picture, request, &folder::candidates(&request.folder))
}

/// Saves `picture` as `request` says, in the first of `folders` that takes it.
fn save_in(picture: &RgbaImage, request: &Request, folders: &[PathBuf]) -> Result<Saved> {
    let bytes = encode(picture, request.format)?;
    let name = naming::file_name(&request.title, request.position, request.format.extension());
    let mut failures = Vec::new();
    for folder in folders {
        match write_new(folder, &name, &bytes) {
            Ok(path) => {
                let instead_of = (folder != &folders[0]).then(|| folders[0].clone());
                if instead_of.is_some() {
                    tracing::warn!("snapshot: saved in {} instead: {failures:?}", folder.display());
                }
                return Ok(Saved { path, instead_of, thumbnail: thumbnail(picture) });
            }
            Err(error) => failures.push(format!("{}: {error:#}", folder.display())),
        }
    }
    bail!("no folder could be written: {}", failures.join("; "))
}

/// `picture` as a file of `format`.
fn encode(picture: &RgbaImage, format: SnapshotFormat) -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    match format {
        SnapshotFormat::Png => picture.write_to(&mut bytes, format.image_format()),
        SnapshotFormat::Jpeg => {
            let rgb = DynamicImage::ImageRgba8(picture.clone()).into_rgb8();
            let mut encoder =
                image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY);
            encoder.encode_image(&rgb)
        }
    }
    .context("couldn't encode the picture")?;
    Ok(bytes.into_inner())
}

/// Writes `bytes` to a new file named `name` in `folder`, made if missing, numbering the name
/// while it is taken.
fn write_new(folder: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    fs::create_dir_all(folder).context("couldn't make the folder")?;
    for attempt in 0..MAX_ATTEMPTS {
        let path = naming::numbered(folder, name, attempt);
        // `create_new` fails rather than overwrite, even when another snapshot races for it.
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let written = file.write_all(bytes).and_then(|()| file.sync_all());
                if let Err(error) = written {
                    fs::remove_file(&path).ok();
                    return Err(error).context("couldn't write the file");
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error).context("couldn't make the file"),
        }
    }
    bail!("every name is taken")
}

/// A small copy of `picture` for the toast, in BGRA as GPUI's `RenderImage` wants.
fn thumbnail(picture: &RgbaImage) -> RgbaImage {
    let (width, height) = picture.dimensions();
    let scale = (THUMBNAIL.0 as f32 / width as f32).min(THUMBNAIL.1 as f32 / height as f32);
    let size = |side: u32| ((side as f32 * scale).round() as u32).max(1);
    let mut small = imageops::resize(picture, size(width), size(height), FilterType::Triangle);
    small.pixels_mut().for_each(|pixel| pixel.0.swap(0, 2));
    small
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use image::Rgba;

    use super::*;

    /// A fresh folder per test, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir()
                .join(format!("rvlc-snapshot-test-{}-{id}", std::process::id()));
            fs::remove_dir_all(&dir).ok();
            fs::create_dir_all(&dir).expect("a scratch folder");
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).ok();
        }
    }

    fn picture() -> RgbaImage {
        RgbaImage::from_pixel(640, 360, Rgba([200, 10, 20, 255]))
    }

    fn request(format: SnapshotFormat) -> Request {
        Request {
            title: "Clip".into(),
            position: Duration::from_millis(1500),
            format,
            folder: String::new(),
        }
    }

    #[test]
    fn saves_a_png_in_a_new_folder_and_numbers_a_second_one() {
        let scratch = Scratch::new();
        let folder = scratch.0.join("missing").join("made");
        let folders = [folder.clone()];
        let first = save_in(&picture(), &request(SnapshotFormat::Png), &folders).expect("saved");
        assert_eq!(first.path, folder.join("Clip_00-00-01.500.png"));
        assert!(first.instead_of.is_none());
        let reread = image::open(&first.path).expect("a PNG").to_rgba8();
        assert_eq!(reread.dimensions(), (640, 360));
        assert_eq!(reread.get_pixel(0, 0), &Rgba([200, 10, 20, 255]));
        let second = save_in(&picture(), &request(SnapshotFormat::Png), &folders).expect("saved");
        assert_eq!(second.path, folder.join("Clip_00-00-01.500 (2).png"));
        assert_eq!(first.thumbnail.dimensions(), (192, 108));
        assert_eq!(first.thumbnail.get_pixel(0, 0), &Rgba([20, 10, 200, 255]), "BGRA");
    }

    #[test]
    fn saves_a_jpeg() {
        let scratch = Scratch::new();
        let saved =
            save_in(&picture(), &request(SnapshotFormat::Jpeg), std::slice::from_ref(&scratch.0))
                .expect("saved");
        assert_eq!(saved.path.extension().and_then(|e| e.to_str()), Some("jpg"));
        assert_eq!(image::open(&saved.path).expect("a JPEG").width(), 640);
    }

    #[test]
    fn a_folder_that_cant_be_made_falls_back_and_says_so() {
        let scratch = Scratch::new();
        let file = scratch.0.join("a file");
        fs::write(&file, b"not a folder").expect("written");
        let wanted = file.join("inside");
        let folders = [wanted.clone(), scratch.0.clone()];
        let saved = save_in(&picture(), &request(SnapshotFormat::Png), &folders).expect("saved");
        assert_eq!(saved.instead_of, Some(wanted));
        assert_eq!(saved.path.parent(), Some(scratch.0.as_path()));
    }

    #[test]
    fn no_folder_at_all_is_an_error() {
        let scratch = Scratch::new();
        let file = scratch.0.join("a file");
        fs::write(&file, b"not a folder").expect("written");
        assert!(
            save_in(&picture(), &request(SnapshotFormat::Png), &[file.join("inside")]).is_err()
        );
    }
}
