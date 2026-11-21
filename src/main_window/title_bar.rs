//! The main window's player-style title bar: logo, `…` menu, media folder, the
//! centered title, and the caption buttons.

use gpui::{AnyElement, ClickEvent, Context, Div, SharedString, Window, div, prelude::*, px, svg};

use super::{MainWindow, TITLE_BAR_HEIGHT, file_name};
use crate::assets::icons;
use crate::settings;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::popover::{self, Side};
use crate::window_chrome::{self, Caption, TRAFFIC_LIGHTS_WIDTH};

/// Title bar buttons on the left: 28px square, 4px apart.
const BAR_BUTTON: f32 = 28.;
const BAR_PAD: f32 = 8.;

impl MainWindow {
    pub(super) fn render_title_bar(
        &self,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let caption = window_chrome::caption_buttons(
            &[Caption::Minimize, Caption::Maximize, Caption::Fullscreen, Caption::Close],
            window.is_maximized(),
            p,
        );

        window_chrome::drag_bar("title-bar", TITLE_BAR_HEIGHT)
            .bg(p.title_bar)
            .justify_between()
            .child(self.render_left_group(p, cx))
            .child(div().flex().flex_row().h_full().children(caption))
            .child(self.render_centered_title(p))
            .into_any_element()
    }

    /// The logo, the `…` menu button and, once one is chosen, the media folder.
    fn render_left_group(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let left_inset = if cfg!(target_os = "macos") { TRAFFIC_LIGHTS_WIDTH } else { BAR_PAD };
        let folder = settings::get(cx)
            .local
            .media_folders
            .first()
            .map(|folder| SharedString::from(file_name(folder)));

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(4.))
            .pl(px(left_inset))
            .child(logo(p))
            .child(self.render_menu_button(cx))
            .when_some(folder, |left, folder| {
                left.child(div().mx(px(6.)).w(px(1.)).h(px(16.)).bg(p.divider))
                    .child(media_folder(p, folder))
            })
    }

    fn render_menu_button(&self, cx: &mut Context<Self>) -> Div {
        div().relative().child(self.menu_button.element()).child(
            IconButton::new("menu", icons::MORE, "Menu")
                .size(BAR_BUTTON, 18.)
                .selected(self.menu.is_some())
                .in_title_bar()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle_menu(window, cx);
                })),
        )
    }

    /// The window title, centered on the window rather than between the side groups.
    fn render_centered_title(&self, p: &Palette) -> Div {
        div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .max_w(px(480.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_size(px(13.))
                    .text_color(p.text_strong)
                    .child(SharedString::from(self.title())),
            )
    }

    /// The `…` menu, below its button.
    pub(super) fn render_menu(&self) -> Option<AnyElement> {
        let (menu, _) = self.menu.as_ref()?;
        let button = self.menu_button.bounds()?;
        Some(popover::attached(button, Side::Below, 4., 1, menu.clone()))
    }
}

fn logo(p: &Palette) -> Div {
    div()
        .size(px(BAR_BUTTON))
        .flex()
        .items_center()
        .justify_center()
        .child(svg().path(icons::LOGO_SMALL).size(px(18.)).text_color(p.accent_fg))
}

/// The folder the library opens on (Stage 4).
fn media_folder(p: &Palette, folder: SharedString) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(6.))
        .px(px(4.))
        .child(svg().path(icons::FOLDER).size(px(18.)).text_color(p.icon))
        .child(div().text_size(px(13.)).text_color(p.text).child(folder))
}
