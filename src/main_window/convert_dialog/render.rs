//! How the Convert dialog looks: Files, Format, Destination and, once anything was asked for,
//! the Conversions, in the shared frame ([`crate::main_window::device_dialog`]).

use gpui::{ClickEvent, Context, Div, IntoElement, SharedString, Window, div, prelude::*, px};

use super::ConvertDialog;
use crate::app::media::Media;
use crate::assets::icons;
use crate::convert::Destination;
use crate::main_window::device_dialog::{FIELD_GAP, Frame, controls, note, section};
use crate::theme::text_px;
use crate::theme::{self, Palette};
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::metrics;
use crate::ui::segmented::Segmented;

/// The format list's width.
const FORMAT_WIDTH: f32 = 320.;
/// The most files listed before the list scrolls.
const FILES_HEIGHT: f32 = 160.;
/// A listed file's row height.
const ROW_HEIGHT: f32 = 32.;

impl Render for ConvertDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let mut sections = vec![
            self.files_section(&p, cx),
            self.format_section(&p, cx),
            self.destination_section(&p, cx),
        ];
        sections.extend(self.queue_section(&p, cx));
        let frame = Frame {
            id: "convert",
            title: "Convert",
            sections,
            can_play: !self.sources.is_empty(),
            buttons: ("Close", "Start"),
        };
        div().track_focus(&self.focus).child(frame.render(&p, window, cx, Self::start))
    }
}

impl ConvertDialog {
    /// Files: what Start converts, each with a button taking it off, and Add Files….
    fn files_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let description: SharedString = match self.sources.len() {
            0 => "Add the files to convert.".into(),
            1 => "One file to convert.".into(),
            count => format!("{count} files to convert, a couple at a time.").into(),
        };
        let rows: Vec<_> = self
            .sources
            .iter()
            .enumerate()
            .map(|(index, media)| source_row(index, media, p, cx))
            .collect();
        let list = div()
            .id("convert-files")
            .w_full()
            .max_h(px(FILES_HEIGHT))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .children(rows);
        let add = Button::new("convert-add-files", "Add Files…")
            .style(ButtonStyle::Filled)
            .icon(icons::ADD)
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| Self::add_files(window, cx)));
        let body = div().flex().flex_col().items_start().gap(px(FIELD_GAP)).child(list).child(add);
        section("Files", description, p).child(controls(body.w_full()))
    }

    /// Format: the presets, and that audio ones keep only the sound.
    fn format_section(&self, p: &Palette, cx: &Context<Self>) -> Div {
        let preset = self.preset(cx);
        let description = if preset.is_audio_only() {
            "Only the sound is kept."
        } else {
            "The container and codecs the copies get. Subtitles aren't kept."
        };
        section("Format", description, p)
            .child(controls(div().w(px(FORMAT_WIDTH)).child(self.presets.clone())))
    }

    /// Destination: next to each file, or a folder picked, shown with Change….
    fn destination_section(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let in_folder = matches!(self.destination, Destination::Folder(_));
        let options = ["Next to each file", "In a folder"];
        let choice =
            Segmented::new("convert-destination", "Destination", options, usize::from(in_folder))
                .on_select(cx.listener(|this, index: &usize, window, cx| {
                    this.set_destination(*index == 1, window, cx);
                }));
        let detail = match &self.destination {
            Destination::NextToSource => {
                note("Streams and discs go to your Videos or Music folder.", p)
            }
            Destination::Folder(folder) => {
                let change = Button::new("convert-change-folder", "Change…")
                    .style(ButtonStyle::Filled)
                    .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                        Self::pick_folder(window, cx);
                    }));
                let path = div().min_w_0().truncate().child(folder.display().to_string());
                let row = div().flex().items_center().gap(px(10.)).pt(px(8.));
                row.child(change).child(path).into_any_element()
            }
        };
        let description = "Copies never replace a file: a name that's taken gets a number.";
        section("Destination", description, p).child(controls(choice)).child(detail)
    }
}

/// A file to convert: its name, its folder, and a button taking it off the list.
fn source_row(index: usize, media: &Media, p: &Palette, cx: &mut Context<ConvertDialog>) -> Div {
    let folder =
        media.as_path().and_then(|path| path.parent()).map(|folder| folder.display().to_string());
    let name = media.display_name();
    let remove = IconButton::new(("convert-remove", index), icons::CLOSE, format!("Remove {name}"))
        .size(24., 14.)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.remove_source(index, cx)));
    div()
        .h(px(ROW_HEIGHT))
        .flex_none()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(div().flex_none().max_w(px(FORMAT_WIDTH)).truncate().child(name))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(text_px(metrics::TEXT_SMALL))
                .text_color(p.text_muted)
                .children(folder),
        )
        .child(remove)
}
