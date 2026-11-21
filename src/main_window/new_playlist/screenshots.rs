//! A screenshot test of the empty dialog against `dialogs/new_playlist/new_playlist.png`.

use gpui::{AppContext as _, Context, Entity, IntoElement, Render, Window, div, prelude::*, px};

use super::NewPlaylistDialog;
use crate::screenshot_test::{self, Shot};
use crate::ui::modal::ModalLayer;

/// The design PNG's size, in logical pixels, and where the dialog sits in it.
const DESIGN_SIZE: (f32, f32) = (743., 619.);
const DIALOG_TOP: f32 = 7.;
/// The share of pixels allowed to differ: the text, as the design uses Inter, and the design's
/// drop shadow. Measured near 2% on macOS (November 2026).
const MAX_MISMATCH: f64 = 0.04;

/// The dialog where the design draws it.
struct DialogShot {
    dialog: Entity<NewPlaylistDialog>,
}

impl Render for DialogShot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(DIALOG_TOP))
            .child(self.dialog.clone())
    }
}

#[test]
fn the_empty_dialog_matches_the_design() {
    let shot = Shot {
        name: "new-playlist",
        design: "docs/ui/dialogs/new_playlist/new_playlist.png".into(),
        design_origin: (0, 0),
        window: DESIGN_SIZE,
        skip_top: 0.,
        max_mismatch: MAX_MISMATCH,
    };
    let failure = screenshot_test::check(&shot, |window, cx| {
        let modals = cx.new(|_| ModalLayer::new()).downgrade();
        let dialog = cx.new(|cx| {
            NewPlaylistDialog::new(Vec::new(), modals, |_, _, _| {}, window, cx)
                .without_window_margin()
        });
        cx.new(|_| DialogShot { dialog })
    });
    assert!(failure.is_none(), "{}", failure.unwrap_or_default());
}
