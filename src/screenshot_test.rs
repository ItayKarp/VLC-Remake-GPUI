//! Screenshot tests (S0.8.3): render a view off screen with the platform's real text
//! system and GPU renderer, then compare it with its design PNG.
//!
//! Every run writes `<name>.actual.png`, `<name>.expected.png` and `<name>.diff.png`
//! (mismatched pixels in red) to `target/screenshots/`, pass or fail, so a design check
//! is a matter of opening that folder.
//!
//! The comparison is tolerant, because the designs use Inter and the app uses the system
//! font: each shot allows a share of mismatched pixels, set a little above what it measures
//! today so a layout or color regression fails while font hinting doesn't.
//!
//! GPUI has a headless renderer on macOS (Metal) and Linux (wgpu, which needs a GPU or a
//! software Vulkan driver). Elsewhere the tests print a note and pass, unless
//! `RVLC_SCREENSHOTS=require` is set.

mod compare;

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{App, Entity, HeadlessAppContext, PlatformTextSystem, Render, Window, px, size};
use image::RgbaImage;

use compare::Comparison;

/// The scale the design PNGs are exported at.
const DESIGN_SCALE: f32 = 2.;

/// One screen to render and the design it should match.
pub struct Shot {
    /// File stem for the written images; unique per shot.
    pub name: &'static str,
    /// The design PNG, relative to the crate root.
    pub design: String,
    /// Top-left corner of the window's content in the design PNG, in its pixels.
    pub design_origin: (u32, u32),
    /// The window's logical size.
    pub window: (f32, f32),
    /// Logical rows at the top of the window that the design doesn't show (our title strip).
    pub skip_top: f32,
    /// The share of pixels (0 to 1) allowed to differ from the design.
    pub max_mismatch: f64,
}

/// Renders the view `build` makes as `shot` describes and compares it with the design.
/// Returns a failure message, or `None` when it matches or this platform can't render.
pub fn check<V: Render>(
    shot: &Shot,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> Option<String> {
    let Some(mut cx) = headless_context() else {
        println!("screenshot {}: skipped, no headless renderer on this platform", shot.name);
        return None;
    };
    let actual = render(&mut cx, shot, build);
    let actual = crop_top(&actual, shot);
    let expected = design_region(shot, actual.width(), actual.height());
    let comparison = Comparison::new(&actual, &expected);
    let written = write_images(shot.name, &actual, &expected, &comparison.diff);
    let mismatch = comparison.mismatch;
    println!("screenshot {}: {:.2}% of pixels differ", shot.name, mismatch * 100.);
    (mismatch > shot.max_mismatch).then(|| {
        format!(
            "{}: {:.2}% of pixels differ from {} (allowed {:.2}%); see {}",
            shot.name,
            mismatch * 100.,
            shot.design,
            shot.max_mismatch * 100.,
            written.display(),
        )
    })
}

/// A headless app with the real text system, assets and the app's globals, or `None`
/// when this platform has no headless renderer.
fn headless_context() -> Option<HeadlessAppContext> {
    match gpui_platform::current_headless_renderer() {
        Ok(Some(_)) => {}
        Ok(None) => return skip_or_fail("GPUI has no headless renderer here"),
        Err(error) => return skip_or_fail(&format!("no headless renderer: {error:#}")),
    }
    let mut cx = HeadlessAppContext::with_platform(
        text_system(),
        Arc::new(crate::assets::Assets),
        gpui_platform::current_headless_renderer,
    );
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
    });
    Some(cx)
}

/// Skips, unless `RVLC_SCREENSHOTS=require` asks for screenshots to run everywhere.
fn skip_or_fail<T>(reason: &str) -> Option<T> {
    assert!(
        std::env::var("RVLC_SCREENSHOTS").as_deref() != Ok("require"),
        "RVLC_SCREENSHOTS=require, but {reason}"
    );
    None
}

/// The text system the app uses on this platform, so text is shaped as in the app.
/// Built directly: the platform that owns it in the app must be made on the main thread.
fn text_system() -> Arc<dyn PlatformTextSystem> {
    #[cfg(target_os = "macos")]
    return Arc::new(gpui_apple::AppleTextSystem::new());
    #[cfg(target_os = "linux")]
    return Arc::new(gpui_wgpu::CosmicTextSystem::new(LINUX_FALLBACK_FONT));
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    return Arc::new(gpui::NoopTextSystem::new());
}

/// The font GPUI's Linux platform falls back to.
#[cfg(target_os = "linux")]
const LINUX_FALLBACK_FONT: &str = "IBM Plex Sans";

/// Opens `shot`'s window, paints it and reads back the frame.
fn render<V: Render>(
    cx: &mut HeadlessAppContext,
    shot: &Shot,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> RgbaImage {
    let window = cx
        .open_window(size(px(shot.window.0), px(shot.window.1)), build)
        .expect("the test platform always opens windows");
    // Paint twice: the first frame loads images and lays out text the second one uses.
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
            .expect("the window was just opened");
    }
    cx.capture_screenshot(window.into()).expect("a headless renderer is configured")
}

/// Drops the rows the design doesn't show.
fn crop_top(image: &RgbaImage, shot: &Shot) -> RgbaImage {
    let scale = image.width() as f32 / shot.window.0;
    let top = (shot.skip_top * scale).round() as u32;
    image::imageops::crop_imm(image, 0, top, image.width(), image.height() - top).to_image()
}

/// The window's area of the design, scaled to `width` × `height`.
fn design_region(shot: &Shot, width: u32, height: u32) -> RgbaImage {
    let path = crate_root().join(&shot.design);
    let design = image::open(&path)
        .unwrap_or_else(|error| panic!("can't read {}: {error}", path.display()))
        .to_rgba8();
    let (x, y) = shot.design_origin;
    let region_width = (shot.window.0 * DESIGN_SCALE) as u32;
    let region_height = ((shot.window.1 - shot.skip_top) * DESIGN_SCALE) as u32;
    let region_width = region_width.min(design.width() - x);
    let region_height = region_height.min(design.height() - y);
    let region = image::imageops::crop_imm(&design, x, y, region_width, region_height).to_image();
    if region.dimensions() == (width, height) {
        return region;
    }
    image::imageops::resize(&region, width, height, image::imageops::FilterType::Triangle)
}

/// Writes the three images for `name` and returns the folder they're in.
fn write_images(name: &str, actual: &RgbaImage, expected: &RgbaImage, diff: &RgbaImage) -> PathBuf {
    let folder = target_dir().join("screenshots");
    std::fs::create_dir_all(&folder).expect("the target folder is writable");
    for (kind, image) in [("actual", actual), ("expected", expected), ("diff", diff)] {
        let path = folder.join(format!("{name}.{kind}.png"));
        image.save(&path).unwrap_or_else(|error| panic!("can't write {}: {error}", path.display()));
    }
    folder
}

/// The crate root, which design paths are relative to.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Cargo's target folder, honoring `CARGO_TARGET_DIR`.
fn target_dir() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| crate_root().join("target"), PathBuf::from)
}
