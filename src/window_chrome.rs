//! The custom (borderless-looking) title bars.
//!
//! - macOS: the native traffic lights are drawn by the OS over the bar.
//! - Windows / Linux: we draw the caption buttons, and mark them with
//!   `WindowControlArea` so Windows gets native hit-testing (snap layouts on
//!   Maximize, hover behaviour, double-click on the bar to maximize, etc.).
//!
//! [`title_bar`] is onboarding's plain strip (Minimize, Close). [`drag_bar`] and
//! [`caption_buttons`] build the main window's player-style bar (S0.3.4).

use std::cell::Cell;

use gpui::{
    AnyElement, Div, MouseButton, Stateful, Window, WindowControlArea, div, prelude::*, px, svg,
};

use crate::assets::icons;
use crate::theme::Palette;

pub const TITLE_BAR_HEIGHT: f32 = 32.;

/// Width the macOS traffic lights take at the left of the bar.
pub const TRAFFIC_LIGHTS_WIDTH: f32 = 78.;

/// A caption button at the right of the bar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Caption {
    Minimize,
    Maximize,
    Fullscreen,
    Close,
}

/// Onboarding's title strip: drag area plus Minimize and Close.
pub fn title_bar(palette: &Palette) -> Stateful<Div> {
    drag_bar("title-bar", TITLE_BAR_HEIGHT)
        .justify_end()
        .children(caption_buttons(
            &[Caption::Minimize, Caption::Close],
            false,
            palette,
        ))
}

thread_local! {
    /// Set on mouse down on a drag bar, cleared on mouse up. The move starts on the
    /// first mouse move, so a double-click still reaches us on macOS and Linux.
    static SHOULD_MOVE: Cell<bool> = const { Cell::new(false) };
}

/// An absolutely positioned, full-width bar that drags the window, and maximizes
/// (Windows, Linux) or does the user's double-click action (macOS) on double-click.
/// Children that should be clickable must `.occlude()`.
pub fn drag_bar(id: &'static str, height: f32) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .top_0()
        .left_0()
        .w_full()
        .h(px(height))
        .flex()
        .flex_row()
        .items_center()
        // Windows drags, double-clicks and snaps natively through this area.
        .window_control_area(WindowControlArea::Drag)
        .when(!cfg!(target_os = "windows"), |bar| {
            bar.on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count >= 2 {
                    SHOULD_MOVE.set(false);
                    double_click(window);
                } else {
                    SHOULD_MOVE.set(true);
                }
            })
            .on_mouse_up(MouseButton::Left, |_, _, _| SHOULD_MOVE.set(false))
            .on_mouse_up_out(MouseButton::Left, |_, _, _| SHOULD_MOVE.set(false))
            .on_mouse_move(|_, window, _| {
                if SHOULD_MOVE.replace(false) {
                    window.start_window_move();
                }
            })
        })
}

fn double_click(window: &Window) {
    if cfg!(target_os = "macos") {
        window.titlebar_double_click();
    } else {
        window.zoom_window();
    }
}

/// The caption buttons, in order. Empty on macOS, where the traffic lights do this.
/// `maximized` swaps the Maximize icon for Restore.
pub fn caption_buttons(buttons: &[Caption], maximized: bool, palette: &Palette) -> Vec<AnyElement> {
    if cfg!(target_os = "macos") {
        return Vec::new();
    }
    buttons
        .iter()
        .map(|&button| caption_button(button, maximized, palette))
        .collect()
}

fn caption_button(button: Caption, maximized: bool, palette: &Palette) -> AnyElement {
    let (id, icon, area) = match button {
        Caption::Minimize => ("minimize", icons::MINIMIZE, Some(WindowControlArea::Min)),
        Caption::Maximize => (
            "maximize",
            if maximized {
                icons::RESTORE
            } else {
                icons::MAXIMIZE
            },
            Some(WindowControlArea::Max),
        ),
        // Not a native caption button, so it's an ordinary click target.
        Caption::Fullscreen => ("fullscreen", icons::FULLSCREEN, None),
        Caption::Close => ("close", icons::CLOSE, Some(WindowControlArea::Close)),
    };
    let is_close = button == Caption::Close;
    let hover_bg = if is_close {
        palette.close_hover
    } else {
        palette.button_hover
    };
    let hover_icon = palette.close_hover_icon;

    div()
        .id(id)
        .group(id)
        .w(px(46.))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .when_some(area, |this, area| this.window_control_area(area))
        .hover(move |style| style.bg(hover_bg))
        // Linux has no native hit-testing for these areas, and Fullscreen has none
        // anywhere, so handle clicks too.
        .when(
            !cfg!(target_os = "windows") || button == Caption::Fullscreen,
            |this| {
                this.on_click(move |_, window, _| match button {
                    Caption::Close => window.remove_window(),
                    Caption::Minimize => window.minimize_window(),
                    Caption::Maximize => window.zoom_window(),
                    Caption::Fullscreen => window.toggle_fullscreen(),
                })
            },
        )
        .child(
            svg()
                .path(icon)
                .size(px(16.))
                .text_color(palette.icon)
                .when(is_close, |this| {
                    this.group_hover(id, |style| style.text_color(hover_icon))
                }),
        )
        .into_any_element()
}
