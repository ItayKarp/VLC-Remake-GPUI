//! The main window's player title bar (S2.1.1): the logo, the `…` menu, the sidebar toggle, a
//! divider, the folder chip, the centered title, and the caption buttons (native traffic lights
//! on macOS). The bar drags the window and maximizes on double-click; its buttons don't.
//!
//! What follows the logo comes from the player layout (S8.7): Customize Player can move these
//! items to the control bar and bring others here. Music mode keeps the designed title bar, as
//! it keeps its own control bar.

mod fit;
pub(super) mod text;

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, ClickEvent, Context, Div, MouseButton, SharedString, Stateful, TextRun,
    Window, black, div, prelude::*, px, svg,
};

use super::control_bar::{self, Place, divider};
use super::{MainWindow, TITLE_BAR_HEIGHT};
use crate::app::media::Media;
use crate::assets::icons;
use crate::db::metadata::MediaKind;
use crate::keymap;
use crate::keymap::actions::ToggleFileBrowser;
use crate::library::{self, folder_view::kind_by_extension};
use crate::player_layout::{Item, Layout, Piece, Zone};
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
/// The control bar's icon size, for the title bar's buttons moved there.
const CONTROL_BAR_ICON: f32 = control_bar::ICON;
/// The widest the folder chip gets in the control bar.
const CHIP_IN_BAR: f32 = control_bar::CHIP_WIDTH;
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
        let pieces = self.title_bar_pieces();
        let title = SharedString::from(self.title(cx));
        // In edit mode the title is in line with the other pieces, to be moved.
        let shows_title = pieces.contains(&Piece::Item(Item::Title)) && !self.is_customizing();
        // Full library mode shows where the library is in its own breadcrumb (`player_03`).
        let folder = self.chip_folder(cx).filter(|_| !self.library_panel.is_full());
        let folder = folder.filter(|_| pieces.contains(&Piece::Item(Item::Folder)));
        let chip_name = folder.as_ref().map(|(folder, _)| folder_name(folder));
        let fitted = fit::fit(&fit::Widths {
            window: f32::from(window.viewport_size().width),
            left: left_inset() + BAR_BUTTON + items_width(&pieces, cx),
            chip: chip_name.as_ref().map(|name| chip_width(name, window)),
            right: if cfg!(target_os = "macos") { 0. } else { CAPTION_WIDTH * 4. },
            title: if shows_title { text_width(&title, window) } else { 0. },
        });
        let chip = folder.zip(chip_name).filter(|_| fitted.chip);
        let caption = window_chrome::caption_buttons(&CAPTIONS, window.is_maximized(), p);

        window_chrome::drag_bar("title-bar", TITLE_BAR_HEIGHT)
            .bg(p.title_bar)
            .justify_between()
            .child(self.render_left_group(&pieces, p, chip, cx))
            .child(div().flex().flex_row().h_full().children(caption))
            .children(shows_title.then(|| centered_title(p, title, fitted.title_max)))
            .into_any_element()
    }

    /// What the title bar shows after the logo.
    fn title_bar_pieces(&self) -> Vec<Piece> {
        if self.is_customizing() {
            self.shown_layout().title_bar.clone()
        } else if self.is_music_mode() {
            Layout::designed().title_bar
        } else {
            self.control_bar.layout().title_bar.clone()
        }
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

    /// The folder the chip names and the kind of media it holds: the loaded file's (none for a
    /// URL), or with nothing loaded the first media folder.
    fn chip_folder(&self, cx: &App) -> Option<(PathBuf, MediaKind)> {
        let player = self.player.as_ref().map(|(player, _)| player.read(cx));
        let loaded = player.filter(|player| player.state().is_active()).and_then(|p| p.media());
        if let Some(media) = loaded {
            let file = media.as_path()?;
            return Some((file.parent()?.to_path_buf(), kind_by_extension(file)));
        }
        let folder = settings::get(cx).local.media_folders.first().cloned()?;
        Some((folder, MediaKind::Video))
    }

    /// The logo, then the layout's title bar items: by design the `…` menu button, the sidebar
    /// toggle and, when there's room, the folder. Spacers push what follows them to the right.
    fn render_left_group(
        &self,
        pieces: &[Piece],
        p: &Palette,
        chip: Option<((PathBuf, MediaKind), SharedString)>,
        cx: &mut Context<Self>,
    ) -> Div {
        let group = div()
            .flex()
            .flex_row()
            .items_center()
            .flex_1()
            .h_full()
            .gap(px(BAR_GAP))
            .pl(px(left_inset()))
            .child(logo(p));
        if self.is_customizing() {
            let pieces = self.render_edit_pieces(Zone::TitleBar, Place::TitleBar, p, cx);
            return Self::edit_drop_zone(group, Zone::TitleBar, cx).children(pieces);
        }
        let mut chip = chip.filter(|_| library::is_enabled(cx));
        let items = pieces.iter().map(|piece| match piece {
            Piece::Item(Item::Folder) => match chip.take() {
                Some((folder, name)) => div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .child(div().mx(px(DIVIDER_MARGIN)).w(px(1.)).h(px(16.)).bg(p.divider))
                    .child(folder_chip(p, folder, name, cx))
                    .into_any_element(),
                None => div().into_any_element(),
            },
            Piece::Item(item) => self.render_item(*item, Place::TitleBar, p, cx),
            Piece::Spacer => div().flex_1().into_any_element(),
            Piece::Divider => divider(p).mx(px(DIVIDER_MARGIN)).into_any_element(),
            Piece::Gap(width) => div().w(px(*width)).flex_none().into_any_element(),
        });
        group.children(items.collect::<Vec<_>>())
    }

    /// The `…` menu button, in the bar `place`.
    pub(super) fn render_menu_button(&self, place: Place, cx: &mut Context<Self>) -> Div {
        let button = IconButton::new("menu", icons::MORE, "Menu")
            .size(BAR_BUTTON, icon_size(place))
            .selected(self.menu.is_some())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_menu(window, cx);
            }));
        let button = if place == Place::TitleBar { button.in_title_bar() } else { button };
        div().relative().flex_none().child(self.menu_button.element()).child(button)
    }

    /// Shows and hides the library sidebar (S4.5), lit while it shows.
    pub(super) fn sidebar_toggle(&self, place: Place, cx: &mut Context<Self>) -> IconButton {
        let button = IconButton::new("sidebar", icons::SIDEBAR, "Show / hide file browser")
            .size(BAR_BUTTON, icon_size(place))
            .selected(self.library_panel.is_open())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_library_panel(window, cx);
            }));
        let button = if place == Place::TitleBar { button.in_title_bar() } else { button };
        match keymap::shortcut(&ToggleFileBrowser, cx) {
            Some(shortcut) => button.shortcut(shortcut),
            None => button,
        }
    }

    /// The folder chip moved out of the title bar, if there's a folder to show.
    pub(super) fn render_folder_chip(
        &self,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let folder = self.chip_folder(cx).filter(|_| library::is_enabled(cx))?;
        let name = folder_name(&folder.0);
        let chip = folder_chip(p, folder, name, cx);
        Some(
            div()
                .flex_none()
                .max_w(px(CHIP_IN_BAR))
                .overflow_hidden()
                .child(chip)
                .into_any_element(),
        )
    }

    /// The `…` menu, below its button, or above it in the control bar.
    pub(super) fn render_menu(&self) -> Option<AnyElement> {
        let (menu, _) = self.menu.as_ref()?;
        let button = self.menu_button.bounds()?;
        let in_title_bar = self.title_bar_pieces().contains(&Piece::Item(Item::Menu));
        let side = if in_title_bar { Side::Below } else { Side::Above };
        Some(popover::attached(button, side, 4., 1, menu.clone()))
    }
}

/// The icons' size in the bar `place`.
fn icon_size(place: Place) -> f32 {
    match place {
        Place::TitleBar => BAR_ICON,
        Place::ControlBar => CONTROL_BAR_ICON,
    }
}

/// How wide the title bar's `pieces` are, without the folder chip and the title, which are
/// measured apart, and with the gap before each.
fn items_width(pieces: &[Piece], cx: &App) -> f32 {
    let width = |piece: &Piece| match piece {
        Piece::Item(Item::Folder | Item::Title) | Piece::Spacer => None,
        Piece::Item(Item::Sidebar) if !library::is_enabled(cx) => None,
        Piece::Item(item) => Some(control_bar::item_width(*item)),
        Piece::Divider => Some(DIVIDER_WIDTH),
        Piece::Gap(width) => Some(*width),
    };
    pieces.iter().filter_map(width).map(|width| BAR_GAP + width).sum()
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

/// The folder chip (S2.1.3): opens the library at the folder (S4.10).
fn folder_chip(
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
