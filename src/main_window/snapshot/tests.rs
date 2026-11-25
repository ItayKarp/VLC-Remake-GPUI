//! Snapshots: why one can't be taken yet, and the toast that says where one was saved, with its
//! Copy putting the picture on the clipboard.

use gpui::{ClipboardEntry, TestAppContext, VisualTestContext};
use image::RgbaImage;

use super::*;
use crate::engine::backend::BackendEvent;
use crate::engine::media_info::VideoSize;
use crate::main_window::now_playing::tests::{draw, open_playing, osd, toasts};
use crate::main_window::tests::{main_view, open};
use crate::opening::scratch::scratch_dir;
use crate::ui::toast::Level;

fn why(cx: &mut VisualTestContext) -> Option<&'static str> {
    main_view(cx).read_with(cx, MainWindow::why_no_snapshot)
}

fn saved(path: PathBuf, instead_of: Option<PathBuf>) -> Saved {
    Saved { path, instead_of, thumbnail: RgbaImage::new(2, 2) }
}

#[gpui::test]
fn a_snapshot_waits_for_a_picture(cx: &mut TestAppContext) {
    let cx = open(cx);
    assert_eq!(why(cx), Some("No media playing"));
    main_view(cx).update(cx, MainWindow::take_snapshot);
    assert_eq!(osd(cx), "No media playing");
}

#[gpui::test]
fn video_without_a_picture_yet_says_so(cx: &mut TestAppContext) {
    let (cx, playing) = open_playing(cx);
    assert_eq!(why(cx), Some("No video to take a snapshot of"));
    let size = VideoSize { width: 320, height: 240 };
    playing.mock.emit(BackendEvent::VideoSize(Some(size)));
    draw(cx);
    assert_eq!(why(cx), Some("No picture yet"));
    main_view(cx).update(cx, MainWindow::take_snapshot);
    assert_eq!(osd(cx), "No picture yet");
}

#[gpui::test]
fn the_toast_says_where_the_snapshot_went(cx: &mut TestAppContext) {
    let cx = open(cx);
    let (folder, wanted) = (PathBuf::from("/pictures"), PathBuf::from("/wanted"));
    let file = folder.join("clip.png");
    main_view(cx).update(cx, |main, cx| {
        main.snapshot_done(Ok(saved(file.clone(), None)), cx);
        main.snapshot_done(Ok(saved(file, Some(wanted))), cx);
        let unreadable = Failure::Unreadable(anyhow::anyhow!("no frame"));
        main.snapshot_done(Err(unreadable), cx);
        let unwritable = Failure::Unwritable(anyhow::anyhow!("read-only"));
        main.snapshot_done(Err(unwritable), cx);
    });
    let shown = toasts(cx);
    assert_eq!(shown[0], (Level::Success, "Snapshot saved in /pictures".into()));
    assert_eq!(
        shown[1],
        (Level::Warning, "Couldn't save in /wanted, so the snapshot is in /pictures".into())
    );
    assert_eq!(shown[2].1, "Couldn't take a snapshot: this video can't be captured.");
    assert_eq!(shown[3].1, "Couldn't save the snapshot: no folder could be written.");
}

#[gpui::test]
fn copy_puts_the_picture_on_the_clipboard(cx: &mut TestAppContext) {
    let cx = open(cx);
    let path = scratch_dir().join("clip.png");
    RgbaImage::new(2, 2).save(&path).expect("a scratch picture");
    cx.update(|_, cx| copy_image(path, cx));
    cx.run_until_parked();
    let copied = cx.update(|_, cx| cx.read_from_clipboard()).expect("something copied");
    assert!(copied.entries().iter().any(|entry| matches!(entry, ClipboardEntry::Image(_))));
}
