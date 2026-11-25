//! Window tests of the Open Capture Device dialog: the devices listed for each source, the
//! format picked, and what it says when the OS refused a device. The cameras are set by the
//! test, so none is opened.

use gpui::{Entity, TestAppContext, VisualTestContext};

use super::{CaptureDialog, Kind};
use crate::capture::{self, Camera, Format, Source};
use crate::main_window::device_dialog::tests::{Handed, draw, open_view, recorder};

/// Opens the dialog, its cameras `cameras` once looked for.
fn open(
    cx: &mut TestAppContext,
    cameras: Vec<Camera>,
) -> (Entity<CaptureDialog>, Handed<crate::app::media::Media>, &mut VisualTestContext) {
    let (handed, on_play) = recorder();
    let (dialog, cx) = open_view(cx, move |window, cx| CaptureDialog::new(on_play, window, cx));
    dialog.update(cx, |dialog, cx| {
        dialog.cameras = Some(cameras);
        dialog.refresh_devices(cx);
    });
    draw(cx);
    (dialog, handed, cx)
}

fn camera() -> Camera {
    Camera { name: "FaceTime HD Camera".into(), id: String::new() }
}

/// Picks choice `index` of each format list.
fn pick_format(index: usize, dialog: &Entity<CaptureDialog>, cx: &mut VisualTestContext) {
    let (sizes, rates) =
        dialog.read_with(cx, |dialog, _| (dialog.sizes.clone(), dialog.rates.clone()));
    sizes.update(cx, |list, cx| list.set_selected(index, cx));
    rates.update(cx, |list, cx| list.set_selected(index, cx));
    draw(cx);
}

fn format(dialog: &Entity<CaptureDialog>, cx: &mut VisualTestContext) -> Format {
    dialog.read_with(cx, CaptureDialog::format)
}

fn source(dialog: &Entity<CaptureDialog>, cx: &mut VisualTestContext) -> Option<Source> {
    dialog.read_with(cx, CaptureDialog::source)
}

#[gpui::test]
fn without_a_camera_nothing_can_play(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, Vec::new());
    assert_eq!(source(&dialog, cx), None);
    dialog.update_in(cx, CaptureDialog::play);
    assert!(handed.borrow().is_empty());
}

#[gpui::test]
fn a_camera_takes_the_size_and_rate_picked(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx, vec![camera()]);
    assert_eq!(source(&dialog, cx), Some(Source::Camera(camera())));
    assert_eq!(format(&dialog, cx), Format::default(), "the device's own to start with");
    pick_format(1, &dialog, cx);
    let expected = Format { size: Some(capture::SIZES[0]), fps: Some(capture::FRAME_RATES[0]) };
    assert_eq!(format(&dialog, cx), expected);
}

#[gpui::test]
fn a_screen_takes_only_the_rate_picked(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx, vec![camera()]);
    dialog.update(cx, |dialog, cx| dialog.set_kind(Kind::Screen, cx));
    pick_format(2, &dialog, cx);
    assert!(matches!(source(&dialog, cx), Some(Source::Screen(_))));
    assert_eq!(format(&dialog, cx), Format { size: None, fps: Some(capture::FRAME_RATES[1]) });
}

#[gpui::test]
fn a_refused_device_says_how_to_allow_it_until_another_is_picked(cx: &mut TestAppContext) {
    let (dialog, _, cx) = open(cx, vec![camera()]);
    let screen = capture::screens().into_iter().next().map(Source::Screen);
    for refused in [Some(Source::Camera(camera())), screen] {
        dialog.update(cx, |dialog, cx| {
            dialog.refused = refused;
            cx.notify();
        });
        draw(cx);
    }
    dialog.update(cx, |dialog, cx| dialog.set_kind(Kind::Camera, cx));
    assert_eq!(dialog.read_with(cx, |dialog, _| dialog.refused.clone()), None);
}

/// Only Linux has no permission to ask for: elsewhere Play could ask the OS for one.
#[cfg(target_os = "linux")]
#[gpui::test]
fn play_plays_the_screen_picked(cx: &mut TestAppContext) {
    let (dialog, handed, cx) = open(cx, Vec::new());
    dialog.update(cx, |dialog, cx| dialog.set_kind(Kind::Screen, cx));
    dialog.update_in(cx, |dialog, window, cx| dialog.play(window, cx));
    draw(cx);
    assert_eq!(handed.borrow().len(), 1);
}
