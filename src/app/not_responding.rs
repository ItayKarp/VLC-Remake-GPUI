//! "VLC is not responding" (S0.6.1): shown by a later launch when the running
//! instance holds the lock but doesn't take the files in time, instead of hanging.

use gpui::{
    App, ClickEvent, Context, FocusHandle, SharedString, TitlebarOptions, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, size,
};

use super::instance::FORWARD_TIMEOUT;
use crate::theme;
use crate::ui::button::Button;

const WIDTH: f32 = 440.;
const HEIGHT: f32 = 200.;

/// The message window's view.
pub struct NotResponding {
    close_button: FocusHandle,
}

/// Opens the message centered on the primary display.
pub fn open(cx: &mut App) -> anyhow::Result<()> {
    let bounds = gpui::Bounds::centered(None, size(px(WIDTH), px(HEIGHT)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            is_resizable: false,
            titlebar: Some(TitlebarOptions {
                title: Some(SharedString::from("VLC")),
                ..Default::default()
            }),
            app_id: Some(super::APP_ID.into()),
            ..Default::default()
        },
        |window, cx| {
            cx.new(|cx| {
                theme::observe(window, cx);
                let close_button = cx.focus_handle().tab_stop(true);
                close_button.focus(window, cx);
                NotResponding { close_button }
            })
        },
    )?;
    Ok(())
}

impl Render for NotResponding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(window, cx);
        let detail = format!(
            "The VLC window that's already open didn't answer within {} seconds, so \
             these files weren't opened. Wait for it to recover, or close it and try again.",
            FORWARD_TIMEOUT.as_secs()
        );
        div()
            .size_full()
            .p(px(24.))
            .flex()
            .flex_col()
            .gap(px(12.))
            .bg(p.background)
            .text_color(p.text)
            .child(
                div().text_size(px(18.)).text_color(p.text_strong).child("VLC is not responding"),
            )
            .child(div().text_size(px(14.)).text_color(p.text_muted).child(detail))
            .child(
                div().flex().justify_end().child(
                    Button::new("close", "Close")
                        .primary()
                        .track_focus(&self.close_button)
                        .on_click(|_: &ClickEvent, _, cx| cx.quit()),
                ),
            )
    }
}
