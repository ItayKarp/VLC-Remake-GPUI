//! The custom (borderless-looking) title strip at the top of the window.
//!
//! - macOS: the native traffic lights are drawn by the OS over this strip.
//! - Windows / Linux: we draw minimize + close buttons, and mark them with
//!   `WindowControlArea` so Windows gets native hit-testing (snap layouts,
//!   hover behaviour, etc.).

use gpui::{
    AnyElement, Div, MouseButton, Stateful, WindowControlArea, div, prelude::*, px, rgb, svg,
};

use crate::assets::icons;
use crate::theme::Palette;

pub const TITLE_BAR_HEIGHT: f32 = 32.;

pub fn title_bar(palette: &Palette) -> Stateful<Div> {
    let bar = div()
        .id("title-bar")
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(TITLE_BAR_HEIGHT))
        .flex()
        .flex_row()
        .justify_end()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |_, window, _| {
            // Windows drags natively through the control area above.
            if !cfg!(target_os = "windows") {
                window.start_window_move();
            }
        });

    if cfg!(target_os = "macos") {
        bar
    } else {
        bar.child(caption_button(
            "minimize",
            icons::MINIMIZE,
            WindowControlArea::Min,
            palette,
        ))
        .child(caption_button(
            "close",
            icons::CLOSE,
            WindowControlArea::Close,
            palette,
        ))
    }
}

fn caption_button(
    id: &'static str,
    icon: &'static str,
    area: WindowControlArea,
    palette: &Palette,
) -> AnyElement {
    let is_close = area == WindowControlArea::Close;
    let hover_bg = if is_close {
        rgb(0xE81123).into()
    } else {
        palette.button_hover
    };
    let icon_color = palette.icon;

    div()
        .id(id)
        .group(id)
        .w(px(46.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .window_control_area(area)
        .hover(move |style| style.bg(hover_bg))
        // Linux has no native hit-testing for these areas, so handle clicks too.
        .when(!cfg!(target_os = "windows"), |this| {
            this.on_click(move |_, window, _| match area {
                WindowControlArea::Close => window.remove_window(),
                WindowControlArea::Min => window.minimize_window(),
                _ => {}
            })
        })
        .child(
            svg()
                .path(icon)
                .size(px(16.))
                .text_color(icon_color)
                .when(is_close, |this| {
                    this.group_hover(id, |style| style.text_color(gpui::white()))
                }),
        )
        .into_any_element()
}
