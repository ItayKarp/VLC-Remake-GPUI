//! The folder chip (S2.1.3): the playing file's folder, or the first media folder, opening it in
//! the library (S4.10). After the logo it sits behind a divider and gives way first when the
//! title needs the room.

use std::path::{Path, PathBuf};

use gpui::{
    ClickEvent, Context, Div, MouseButton, SharedString, Stateful, Window, div, prelude::*, px, svg,
};

use super::{BAR_BUTTON, BAR_GAP, BAR_ICON, DIVIDER_WIDTH, TEXT_SIZE, text_width};
use crate::app::media::Media;
use crate::assets::icons;
use crate::db::metadata::MediaKind;
use crate::main_window::MainWindow;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::{focus, metrics, tooltip};

/// The chip's padding, the gap after its icon, and the widest its name gets.
const CHIP_PAD: f32 = 4.;
const CHIP_GAP: f32 = 6.;
const CHIP_NAME_MAX: f32 = 240.;

/// The folder chip (S2.1.3): opens the library at the folder (S4.10).
pub(super) fn folder_chip(
    p: &Palette,
    (folder, kind): (PathBuf, MediaKind),
    name: SharedString,
    cx: &mut Context<MainWindow>,
) -> Stateful<Div> {
    let hover = p.button_hover;
    let tooltip = folder.display().to_string();
    let chip = div()
        .id("folder-chip")
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(CHIP_GAP))
        .px(px(CHIP_PAD))
        .h(px(BAR_BUTTON))
        .rounded(px(metrics::RADIUS))
        .border_2()
        .border_color(gpui::transparent_black())
        .tab_index(0)
        .cursor_pointer()
        .occlude()
        .hover(move |style| style.bg(hover))
        .tooltip(tooltip::text(tooltip, None::<SharedString>))
        // Pressing the chip must not start a window drag in the bar underneath.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
            this.show_library_folder(&folder, kind, window, cx);
        }))
        .child(svg().path(icons::FOLDER).size(px(BAR_ICON)).flex_none().text_color(p.icon))
        .child(
            div()
                .max_w(px(CHIP_NAME_MAX))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(text_px(TEXT_SIZE))
                .text_color(p.text)
                .child(name),
        );
    focus::ring(chip, p)
}

/// The folder's own name, or its whole path for a drive root.
pub(super) fn folder_name(folder: &Path) -> SharedString {
    Media::File(folder.to_path_buf()).display_name().into()
}

/// The chip's width, with the gap and divider before it.
pub(super) fn chip_width(name: &SharedString, window: &Window) -> f32 {
    let name = text_width(name, window).min(CHIP_NAME_MAX);
    BAR_GAP + DIVIDER_WIDTH + BAR_GAP + 2. * CHIP_PAD + BAR_ICON + CHIP_GAP + name
}
