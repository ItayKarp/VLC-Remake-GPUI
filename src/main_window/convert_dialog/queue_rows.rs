//! The Convert dialog's Conversions section: every conversion asked for this session, each with
//! its file, how it is going (a progress bar while it runs) and a button to cancel it or, once
//! made, show the file; Clear Finished forgets those that are over.

use gpui::{App, ClickEvent, Context, Div, SharedString, div, prelude::*, px, relative};

use super::ConvertDialog;
use crate::assets::icons;
use crate::convert::{Conversions, Job, Status};
use crate::main_window::device_dialog::{FIELD_GAP, controls, section};
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle, IconButton};
use crate::ui::metrics;

/// The most rows shown before the list scrolls.
const LIST_HEIGHT: f32 = 220.;
/// The progress bar's width and thickness.
const BAR_WIDTH: f32 = 140.;
const BAR_HEIGHT: f32 = 4.;
/// The status column's width.
const STATUS_WIDTH: f32 = 190.;

impl ConvertDialog {
    /// Conversions: the queue, once anything was asked for this session.
    pub(super) fn queue_section(&self, p: &Palette, cx: &mut Context<Self>) -> Option<Div> {
        let queue = self.conversions.read(cx).queue();
        let (jobs, unfinished) = (queue.jobs().to_vec(), queue.unfinished());
        if jobs.is_empty() {
            return None;
        }
        let description: SharedString = match unfinished {
            0 => "All done. Closing this window doesn't stop conversions.".into(),
            count => {
                format!("{count} to go. They go on in the background when this window is closed.")
                    .into()
            }
        };
        let any_over = jobs.iter().any(|job| job.status.is_over());
        let rows: Vec<Div> = jobs.iter().map(|job| job_row(job, p, cx)).collect();
        let list = div()
            .id("convert-queue")
            .max_h(px(LIST_HEIGHT))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(rows);
        let clear = any_over.then(|| {
            Button::new("convert-clear", "Clear Finished").style(ButtonStyle::Filled).on_click(
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.conversions.update(cx, Conversions::clear_finished);
                }),
            )
        });
        let body = div().flex().flex_col().items_start().gap(px(FIELD_GAP)).child(list.w_full());
        Some(section("Conversions", description, p).child(controls(body.children(clear))))
    }
}

/// One conversion: the file it makes and what it comes from, how it is going, and its button.
fn job_row(job: &Job, p: &Palette, cx: &mut Context<ConvertDialog>) -> Div {
    let file_name = job.output.file_name().map(|name| name.to_string_lossy().into_owned());
    let source = format!("From {}", job.source.display_name());
    let names = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .child(div().truncate().child(file_name.unwrap_or_default()))
        .child(
            div()
                .truncate()
                .text_size(px(metrics::TEXT_SMALL))
                .text_color(p.text_muted)
                .child(source),
        );
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(names)
        .child(status(&job.status, p))
        .child(job_button(job, cx))
}

/// How a conversion is going: a bar and percentage while it runs, else a word.
fn status(status: &Status, p: &Palette) -> Div {
    let column = || div().flex_none().w(px(STATUS_WIDTH)).text_size(px(metrics::TEXT_SMALL));
    let word = |text: SharedString, color| column().text_color(color).truncate().child(text);
    match status {
        Status::Waiting => word("Waiting".into(), p.text_muted),
        Status::Converting(fraction) => {
            let bar = div()
                .w(px(BAR_WIDTH))
                .h(px(BAR_HEIGHT))
                .rounded_full()
                .bg(p.divider)
                .child(div().h_full().rounded_full().bg(p.accent).w(relative(*fraction)));
            let percent = format!("{:.0} %", fraction * 100.);
            column().flex().items_center().gap(px(8.)).child(bar).child(percent)
        }
        Status::Done => word("Done".into(), p.success),
        Status::Failed(why) => word(why.clone().into(), p.danger),
        Status::Cancelled => word("Cancelled".into(), p.text_muted),
    }
}

/// Cancel while a conversion isn't over, Show in Finder once it made its file.
fn job_button(job: &Job, cx: &mut Context<ConvertDialog>) -> Div {
    let id = job.id;
    let slot = div().flex_none().w(px(28.));
    match &job.status {
        Status::Waiting | Status::Converting(_) => {
            slot.child(IconButton::new(("convert-cancel", id), icons::CLOSE, "Cancel").on_click(
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.conversions.update(cx, |conversions, cx| conversions.cancel(id, cx));
                }),
            ))
        }
        Status::Done => {
            let output = job.output.clone();
            let label = crate::main_window::library_panel::show_in_folder_label();
            slot.child(
                IconButton::new(("convert-reveal", id), icons::FOLDER, label)
                    .on_click(move |_: &ClickEvent, _, cx: &mut App| cx.reveal_path(&output)),
            )
        }
        Status::Failed(_) | Status::Cancelled => slot,
    }
}
