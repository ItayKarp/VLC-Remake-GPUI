//! Home with nothing to show (S4.9.2, not designed): on first run a welcome, with Open File,
//! Add Folder and the media folders' indexing progress; with "Save recently played items" off
//! and no favorites, why Home stays empty.

use gpui::{ClickEvent, Context, Div, div, prelude::*, px};

use super::super::add_menu::{self, Shelf};
use super::super::{INSET, MainWindow};
use crate::assets::icons;
use crate::main_window::library_status;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::Button;
use crate::ui::icon::icon;

/// The icon, the gaps, the text sizes and the air above the block.
const ICON: f32 = 32.;
const GAP: f32 = 8.;
const TITLE_TEXT: f32 = 15.;
const HINT_TEXT: f32 = 13.;
const TOP: f32 = 24.;

/// What the welcome says.
const WELCOME: &str = "Welcome to VLC";
const WELCOME_HINT: &str = "What you watch and listen to shows up here. Open a file to start, \
                            or add a folder to your library.";
/// What Home says with history off and no favorites.
const NOT_SAVED: &str = "Recently played items aren't saved";
const NOT_SAVED_HINT: &str = "Turn on “Save recently played items” in Settings to see what you \
                              were watching here. Favorites show here either way.";

/// The welcome, or with history off why Home is empty.
pub(super) fn render(history: bool, p: &Palette, cx: &mut Context<MainWindow>) -> Div {
    let (glyph, title, hint) = if history {
        (icons::HOME, WELCOME, WELCOME_HINT)
    } else {
        (icons::RECENT, NOT_SAVED, NOT_SAVED_HINT)
    };
    let block = div()
        .pt(px(TOP))
        .px(px(INSET))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(GAP))
        .text_center()
        .child(icon(glyph, ICON, p.text_muted))
        .child(div().text_size(text_px(TITLE_TEXT)).text_color(p.text).child(title))
        .child(div().text_size(text_px(HINT_TEXT)).text_color(p.text_muted).child(hint));
    if !history {
        return block;
    }
    block.child(start_buttons(cx)).children(
        library_status::render(p, cx)
            .map(|roots| div().mt(px(GAP * 2.)).w_full().text_left().child(roots)),
    )
}

/// Open File and Add Folder.
fn start_buttons(cx: &mut Context<MainWindow>) -> Div {
    let open = Button::new("library-home-open", "Open File…").primary().on_click(cx.listener(
        |_, _: &ClickEvent, window, cx| {
            MainWindow::prompt_for_files(window, cx);
        },
    ));
    let main = cx.entity().downgrade();
    let add = Button::new("library-home-add", "Add Folder…").on_click(move |_, _, cx| {
        add_menu::add_folder(main.clone(), Shelf::Videos, cx);
    });
    div().mt(px(GAP)).flex().flex_wrap().justify_center().gap(px(GAP)).child(open).child(add)
}
