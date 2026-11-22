//! The main window's player title bar (S2.1.1): the logo, the `…` menu, the sidebar toggle, a
//! divider, the folder chip, the centered title, and the caption buttons (native traffic lights
//! on macOS). The bar drags the window and maximizes on double-click; its buttons don't.

mod fit;
mod text;

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, MouseButton, SharedString, Stateful, TextRun,
    Window, black, div, prelude::*, px, svg,
};

use super::{MainWindow, TITLE_BAR_HEIGHT};
use crate::app::media::Media;
use crate::assets::icons;
use crate::keymap;
use crate::keymap::actions::ToggleFileBrowser;
use crate::settings;
use crate::theme::Palette;
use crate::ui::button::IconButton;
use crate::ui::popover::{self, Side};
use crate::ui::{focus, metrics, tooltip};
use crate::window_chrome::{self, Caption, TRAFFIC_LIGHTS_WIDTH};

/// Title bar buttons on the left: 28px square, 4px apart.
const BAR_BUTTON: f32 = 28.;
const BAR_GAP: f32 = 4.;
const BAR_PAD: f32 = 8.;
/// The divider before the folder chip: 1px with 6px of air on each side.
const DIVIDER_MARGIN: f32 = 6.;
const DIVIDER_WIDTH: f32 = 1. + 2. * DIVIDER_MARGIN;
const BAR_ICON: f32 = 18.;
const TEXT_SIZE: f32 = 13.;
/// The folder chip: its padding, the gap after its icon, and the widest its name gets.
const CHIP_PAD: f32 = 4.;
const CHIP_GAP: f32 = 6.;
const CHIP_NAME_MAX: f32 = 240.;
/// Each drawn caption button's width (`window_chrome`).
const CAPTION_WIDTH: f32 = 46.;
const CAPTIONS: [Caption; 4] =
    [Caption::Minimize, Caption::Maximize, Caption::Fullscreen, Caption::Close];

impl MainWindow {
    pub(super) fn render_title_bar(
        &self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = SharedString::from(self.title(cx));
        let folder = self.chip_folder(cx);
        let chip_name = folder.as_deref().map(folder_name);
        let fitted = fit::fit(&fit::Widths {
            window: f32::from(window.viewport_size().width),
            left: left_inset() + 3. * BAR_BUTTON + 3. * BAR_GAP,
            chip: chip_name.as_ref().map(|name| chip_width(name, window)),
            right: if cfg!(target_os = "macos") { 0. } else { CAPTION_WIDTH * 4. },
            title: text_width(&title, window),
        });
        let chip = folder.zip(chip_name).filter(|_| fitted.chip);
        let caption = window_chrome::caption_buttons(&CAPTIONS, window.is_maximized(), p);

        window_chrome::drag_bar("title-bar", TITLE_BAR_HEIGHT)
            .bg(p.title_bar)
            .justify_between()
            .child(self.render_left_group(p, chip, cx))
            .child(div().flex().flex_row().h_full().children(caption))
            .child(centered_title(p, title, fitted.title_max))
            .into_any_element()
    }

    /// The title bar's text: what's loaded, or the app's name (S2.1.2).
    pub(super) fn title(&self, cx: &App) -> String {
        let player = self.player.as_ref().map(|(player, _)| player.read(cx));
        let loaded = player.filter(|player| player.state().is_active()).and_then(|player| {
            let media = player.media()?;
            Some(text::Loaded { media, tag: player.metadata().title.as_deref() })
        });
        text::window_title(loaded, settings::get(cx).prefs.interface.show_file_name_in_title_bar)
    }

    /// The folder the chip names: the loaded file's (none for a URL), or with nothing loaded
    /// the first media folder.
    fn chip_folder(&self, cx: &App) -> Option<PathBuf> {
        let player = self.player.as_ref().map(|(player, _)| player.read(cx));
        match player.filter(|player| player.state().is_active()).and_then(|p| p.media()) {
            Some(media) => media.as_path()?.parent().map(Path::to_path_buf),
            None => settings::get(cx).local.media_folders.first().cloned(),
        }
    }

    /// The logo, the `…` menu button, the sidebar toggle and, when there's room, the folder.
    fn render_left_group(
        &self,
        p: &Palette,
        chip: Option<(PathBuf, SharedString)>,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex()
            .flex_row()
            .items_center()
            .flex_none()
            .gap(px(BAR_GAP))
            .pl(px(left_inset()))
            .child(logo(p))
            .child(self.render_menu_button(cx))
            .child(self.sidebar_toggle(cx))
            .when_some(chip, |left, (folder, name)| {
                left.child(div().mx(px(DIVIDER_MARGIN)).w(px(1.)).h(px(16.)).bg(p.divider))
                    .child(folder_chip(p, folder, name))
            })
    }

    fn render_menu_button(&self, cx: &mut Context<Self>) -> Div {
        div().relative().child(self.menu_button.element()).child(
            IconButton::new("menu", icons::MORE, "Menu")
                .size(BAR_BUTTON, BAR_ICON)
                .selected(self.menu.is_some())
                .in_title_bar()
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle_menu(window, cx);
                })),
        )
    }

    /// Shows and hides the library sidebar (S4.5), lit while it shows.
    fn sidebar_toggle(&self, cx: &mut Context<Self>) -> IconButton {
        let button = IconButton::new("sidebar", icons::SIDEBAR, "Show / hide file browser")
            .size(BAR_BUTTON, BAR_ICON)
            .selected(self.library_panel.is_open())
            .in_title_bar()
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_library_panel(window, cx);
            }));
        match keymap::shortcut(&ToggleFileBrowser, cx) {
            Some(shortcut) => button.shortcut(shortcut),
            None => button,
        }
    }

    /// The `…` menu, below its button.
    pub(super) fn render_menu(&self) -> Option<AnyElement> {
        let (menu, _) = self.menu.as_ref()?;
        let button = self.menu_button.bounds()?;
        Some(popover::attached(button, Side::Below, 4., 1, menu.clone()))
    }
}

/// Where the left group starts: after the traffic lights on macOS.
fn left_inset() -> f32 {
    if cfg!(target_os = "macos") { TRAFFIC_LIGHTS_WIDTH } else { BAR_PAD }
}

fn logo(p: &Palette) -> Div {
    div()
        .size(px(BAR_BUTTON))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .child(svg().path(icons::LOGO_SMALL).size(px(BAR_ICON)).text_color(p.accent_fg))
}

/// The folder chip (S2.1.3). The library opens at the folder once it exists (S4.10); until
/// then the chip opens the folder in the system's file manager.
fn folder_chip(p: &Palette, folder: PathBuf, name: SharedString) -> Stateful<Div> {
    let hover = p.button_hover;
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
        .tooltip(tooltip::text(folder.display().to_string(), None::<SharedString>))
        // Pressing the chip must not start a window drag in the bar underneath.
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(move |_, _, cx| cx.open_with_system(&folder))
        .child(svg().path(icons::FOLDER).size(px(BAR_ICON)).flex_none().text_color(p.icon))
        .child(
            div()
                .max_w(px(CHIP_NAME_MAX))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(TEXT_SIZE))
                .text_color(p.text)
                .child(name),
        );
    focus::ring(chip, p)
}

/// The folder's own name, or its whole path for a drive root.
fn folder_name(folder: &Path) -> SharedString {
    Media::File(folder.to_path_buf()).display_name().into()
}

/// The chip's width, with the gap and divider before it.
fn chip_width(name: &SharedString, window: &Window) -> f32 {
    let name = text_width(name, window).min(CHIP_NAME_MAX);
    BAR_GAP + DIVIDER_WIDTH + BAR_GAP + 2. * CHIP_PAD + BAR_ICON + CHIP_GAP + name
}

/// How wide `text` is at the title bar's text size.
fn text_width(text: &SharedString, window: &Window) -> f32 {
    let run = TextRun {
        len: text.len(),
        font: window.text_style().font(),
        color: black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window.text_system().shape_line(text.clone(), px(TEXT_SIZE), &[run], None);
    f32::from(line.width())
}

/// The title, centered on the window rather than between the side groups, and cut with an
/// ellipsis past `max_width`.
fn centered_title(p: &Palette, title: SharedString, max_width: f32) -> Div {
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
                .max_w(px(max_width))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(TEXT_SIZE))
                .text_color(p.text_strong)
                .child(title),
        )
}
