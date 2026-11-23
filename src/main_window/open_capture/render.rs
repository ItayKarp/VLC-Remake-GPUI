//! How the Open Capture Device dialog looks: Source, Device and Format, and, when the OS
//! refused the device, how to allow it, in the shared frame
//! ([`crate::main_window::device_dialog`]).

use gpui::{ClickEvent, Context, Div, IntoElement, Window, div, prelude::*, px};

use super::{CaptureDialog, Kind};
use crate::capture::{self, Source};
use crate::main_window::device_dialog::{Frame, controls, note, section};
use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::segmented::Segmented;

/// Each format list's width.
const FORMAT_WIDTH: f32 = 180.;

impl Render for CaptureDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let mut sections = vec![self.kind_section(&p, cx), self.device_section(&p)];
        sections.push(self.format_section(&p));
        let can_play = self.source(cx).is_some();
        let frame = Frame {
            id: "open-capture",
            title: "Open Capture Device",
            sections,
            can_play,
            buttons: ("Cancel", "Play"),
        };
        div().track_focus(&self.focus).child(frame.render(&p, window, cx, Self::play))
    }
}

impl CaptureDialog {
    /// Source: Camera or Screen.
    fn kind_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let selected = usize::from(self.kind == Kind::Screen);
        let kinds = Segmented::new("capture-kind", "Source", ["Camera", "Screen"], selected)
            .on_select(cx.listener(|this, index: &usize, _, cx| {
                this.set_kind(if *index == 0 { Kind::Camera } else { Kind::Screen }, cx);
            }));
        let description = "A camera or capture card, or what a screen shows.";
        section("Source", description, p).child(controls(kinds))
    }

    /// Device: the cameras or screens, and why one can't be used.
    fn device_section(&self, p: &Palette) -> Div {
        let description = match self.kind {
            Kind::Camera => "Capture cards are listed with the cameras.",
            Kind::Screen => "The whole screen is recorded, as it shows.",
        };
        let device = section("Device", description, p).child(controls(self.devices.clone()));
        match &self.refused {
            Some(source) => device.child(refusal(source, p)),
            None => device,
        }
    }

    /// Format: a camera's size and frame rate, where they can be picked, or a screen's rate.
    fn format_section(&self, p: &Palette) -> Div {
        let list = |list: &gpui::Entity<crate::ui::dropdown::Dropdown>| {
            div().w(px(FORMAT_WIDTH)).child(list.clone())
        };
        let row = div().flex().gap(px(10.));
        match self.kind {
            Kind::Camera if capture::CAMERA_FORMATS => {
                section("Format", "The picture size and frame rate to capture at.", p)
                    .child(controls(row.child(list(&self.sizes)).child(list(&self.rates))))
            }
            Kind::Camera => section("Format", "The camera picks its own size and frame rate.", p),
            Kind::Screen => section("Format", "How many pictures a second to record.", p)
                .child(controls(row.child(list(&self.rates)))),
        }
    }
}

/// Why the OS refused `source`, how to allow it, and a button to its settings page.
fn refusal(source: &Source, p: &Palette) -> Div {
    let text = match source {
        Source::Camera(_) => {
            "VLC isn't allowed to use the camera. Allow it in your system's privacy settings for the camera, then press Play again."
        }
        Source::Screen(_) => {
            "VLC isn't allowed to record the screen. Allow it in your system's Screen Recording settings, then reopen VLC."
        }
    };
    let settings = capture::privacy_settings(source).map(|url| {
        Button::new("open-capture-privacy", "Open Privacy Settings")
            .style(ButtonStyle::Filled)
            .on_click(move |_: &ClickEvent, _, cx| cx.open_url(url))
    });
    div().flex().flex_col().items_start().gap(px(8.)).child(note(text, p)).children(settings)
}
