//! Regenerates the desktop app icons from `assets/app-icon/vlc-logo.svg`.
//!
//! Run with: `cargo run --example gen_icons`
//!
//! Outputs (committed to the repo, so normal builds don't need this step):
//! - `assets/app-icon/icon-{N}.png`  — PNGs used by the macOS bundle and Linux
//! - `assets/app-icon/icon.ico`      — embedded into the Windows .exe by `build.rs`
//! - `assets/app-icon/icon.icns`     — macOS app icon

use std::fs;
use std::io::Cursor;
use std::path::Path;

use resvg::{tiny_skia, usvg};

const SRC: &str = "assets/app-icon/vlc-logo.svg";
const OUT: &str = "assets/app-icon";

fn render(tree: &usvg::Tree, size: u32) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(size, size).expect("pixmap");
    let svg_size = tree.size();
    let scale = size as f32 / svg_size.width().max(svg_size.height());
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().expect("encode png")
}

fn main() {
    let svg = fs::read(SRC).expect("read logo svg");
    let tree = usvg::Tree::from_data(&svg, &usvg::Options::default()).expect("parse svg");
    let out = Path::new(OUT);

    // Standalone PNGs.
    for size in [32, 128, 256, 512, 1024] {
        fs::write(out.join(format!("icon-{size}.png")), render(&tree, size)).unwrap();
    }

    // Windows .ico: every size Explorer / the taskbar may ask for.
    let mut ico = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16, 20, 24, 32, 40, 48, 64, 128, 256] {
        let png = render(&tree, size);
        let image = ico::IconImage::read_png(Cursor::new(png)).unwrap();
        ico.add_entry(ico::IconDirEntry::encode(&image).unwrap());
    }
    ico.write(fs::File::create(out.join("icon.ico")).unwrap())
        .unwrap();

    // macOS .icns.
    let mut icns = icns::IconFamily::new();
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let png = render(&tree, size);
        let image = icns::Image::read_png(Cursor::new(png)).unwrap();
        icns.add_icon(&image).unwrap();
    }
    icns.write(fs::File::create(out.join("icon.icns")).unwrap())
        .unwrap();

    println!("icons written to {OUT}/");
}
