//! The About VLC dialog (S2.9.6; not designed): the app's version, the playback engine and its
//! version, the licenses and the credits, with Copy Debug Info for bug reports. The full list
//! of third-party licenses comes with S9.9.

use gpui::{AnyElement, App, Context, SharedString, Window, div, prelude::*, px};

use super::MainWindow;
use crate::diagnostics::system;
use crate::keymap::actions::CopyDebugInfo;
use crate::theme;
use crate::ui::modal::{Dialog, DialogButton};

/// The space between the dialog's paragraphs.
const PARAGRAPH_GAP: f32 = 12.;

/// What the dialog says, read once when it opens.
struct AboutText {
    version: SharedString,
    engine: SharedString,
}

impl AboutText {
    fn now() -> Self {
        Self {
            version: format!("Version {}", system::VERSION).into(),
            engine: engine_line(&system::libvlc_version()).into(),
        }
    }
}

impl MainWindow {
    /// Opens the About dialog over the window.
    pub(super) fn show_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = AboutText::now();
        let buttons = vec![
            DialogButton::new("Copy Debug Info").on_click(|window, cx| {
                window.dispatch_action(Box::new(CopyDebugInfo), cx);
            }),
            DialogButton::new("Close").primary(),
        ];
        let body = move |window: &mut Window, cx: &mut App| body(&text, window, cx);
        let dialog = cx.new(|cx| Dialog::new("About VLC", body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }
}

fn body(text: &AboutText, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = theme::palette(window, cx);
    let muted = |line: &'static str| div().text_color(p.text_muted).child(line);
    div()
        .flex()
        .flex_col()
        .gap(px(PARAGRAPH_GAP))
        .child(div().flex().flex_col().child(text.version.clone()).child(text.engine.clone()))
        .child(muted(
            "A redesigned VLC media player. This app is released under the MIT License. \
             libVLC is free software by VideoLAN and the VLC authors, under the GNU LGPL 2.1 \
             or later.",
        ))
        .child(muted(
            "Credits: VideoLAN and the VLC authors for libVLC; Zed Industries for GPUI, the \
             interface toolkit (Apache License 2.0).",
        ))
        .into_any_element()
}

/// "Playback engine: libVLC 3.0.23 Vetinari".
fn engine_line(version: &str) -> String {
    format!("Playback engine: libVLC {version}")
}

#[cfg(test)]
mod tests {
    use super::engine_line;

    #[test]
    fn names_the_engine_and_its_version() {
        assert_eq!(engine_line("3.0.23 Vetinari"), "Playback engine: libVLC 3.0.23 Vetinari");
    }
}
