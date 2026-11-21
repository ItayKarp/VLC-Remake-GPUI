//! Regenerates the desktop app icons from `assets/app-icon/vlc-logo.svg`.
//!
//! Run with: `cargo run --example gen_icons`
//!
//! Outputs (committed to the repo, so normal builds don't need this step):
//! - `assets/app-icon/icon-{N}.png`  — PNGs used by the macOS bundle and Linux
//! - `assets/app-icon/icon.ico`      — embedded into the Windows .exe by `build.rs`
//! - `assets/app-icon/icon.icns`     — macOS app icon

use std::error::Error;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use resvg::{tiny_skia, usvg};

/// The logo every icon is rendered from.
const SRC: &str = "assets/app-icon/vlc-logo.svg";
/// The directory the icons are written to.
const OUT: &str = "assets/app-icon";
/// Sizes of the standalone PNGs.
const PNG_SIZES: [u32; 5] = [32, 128, 256, 512, 1024];
/// Every size Explorer and the taskbar may ask for.
const ICO_SIZES: [u32; 9] = [16, 20, 24, 32, 40, 48, 64, 128, 256];
/// Every size the macOS icon family holds.
const ICNS_SIZES: [u32; 7] = [16, 32, 64, 128, 256, 512, 1024];

/// The result of each generation step; any failure aborts the run with its message.
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Renders the logo as a square PNG of `size` pixels.
fn render(tree: &usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let svg_size = tree.size();
    let scale = size as f32 / svg_size.width().max(svg_size.height());
    resvg::render(tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    pixmap.encode_png().expect("encode png")
}

/// Writes the standalone PNGs.
fn write_pngs(tree: &usvg::Tree, out: &Path) -> Result {
    for size in PNG_SIZES {
        fs::write(out.join(format!("icon-{size}.png")), render(tree, size))?;
    }
    Ok(())
}

/// Writes the Windows `.ico`.
fn write_ico(tree: &usvg::Tree, out: &Path) -> Result {
    let mut ico = ico::IconDir::new(ico::ResourceType::Icon);
    for size in ICO_SIZES {
        let image = ico::IconImage::read_png(Cursor::new(render(tree, size)))?;
        ico.add_entry(ico::IconDirEntry::encode(&image)?);
    }
    ico.write(fs::File::create(out.join("icon.ico"))?)?;
    Ok(())
}

/// Writes the macOS `.icns`.
fn write_icns(tree: &usvg::Tree, out: &Path) -> Result {
    let mut icns = icns::IconFamily::new();
    for size in ICNS_SIZES {
        let image = icns::Image::read_png(Cursor::new(render(tree, size)))?;
        icns.add_icon(&image)?;
    }
    icns.write(fs::File::create(out.join("icon.icns"))?)?;
    Ok(())
}

fn main() -> Result {
    let svg = fs::read(SRC)?;
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default())?;
    let out = Path::new(OUT);
    write_pngs(&tree, out)?;
    write_ico(&tree, out)?;
    write_icns(&tree, out)?;
    println!("icons written to {OUT}/");
    Ok(())
}
