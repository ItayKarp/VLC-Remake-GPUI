//! The window controls at the title bar's right end (S8.7.4): Minimize, Maximize, Fullscreen
//! and Close by design, drawn as caption buttons off macOS, with whatever was dropped among
//! them. Customize Player can pull Fullscreen out of the group (`player_customize_08.png`
//! moves it to the control bar), where it is an icon button like the others; the other three
//! stay.
//!
//! On macOS the traffic lights stand in for the group, Fullscreen included (the green button),
//! so it shows only what was dropped into it, and Fullscreen only while customizing, to be
//! pulled out.

use gpui::{AnyElement, App, Context, Div, Window, div, prelude::*, px};

use super::items_width;
use crate::main_window::MainWindow;
use crate::main_window::control_bar::Place;
use crate::player_layout::{Item, Piece, Zone};
use crate::theme::Palette;
use crate::window_chrome::{self, Caption};

/// Each drawn caption button's width (`window_chrome`).
const CAPTION_WIDTH: f32 = 46.;
/// The narrowest the group gets while customizing, so there's a place to drop into it even
/// when it shows nothing (on macOS, once Fullscreen is pulled out).
const EDIT_MIN_WIDTH: f32 = 40.;

impl MainWindow {
    /// The window controls group: `pieces`, the layout's, as drawn outside edit mode, or
    /// outlined and taking drops while customizing.
    pub(super) fn render_window_controls(
        &self,
        pieces: &[Piece],
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let group = div().flex().flex_row().items_center().h_full();
        if self.is_customizing() {
            let pieces = self.render_edit_pieces(Zone::WindowControls, p, window, cx);
            let group = group
                .debug_selector(|| "customize-window-controls".into())
                .min_w(px(EDIT_MIN_WIDTH))
                .justify_end()
                // Clear of the window's rounded corner, where nothing is drawn after it.
                .when(cfg!(target_os = "macos"), |group| group.pr(px(8.)));
            return Self::edit_drop_zone(group, Zone::WindowControls, cx).children(pieces);
        }
        let maximized = window.is_maximized();
        let drawn = pieces.iter().filter_map(|piece| match piece {
            Piece::Item(item) if item.is_window_control() => caption(*item, maximized, p),
            piece => Some(self.render_title_bar_piece(*piece, p, cx)),
        });
        group.children(drawn.collect::<Vec<_>>())
    }

    /// The window control `item` while customizing: as it's drawn outside edit mode, except
    /// that on macOS Fullscreen shows as an icon button, to be pulled out. None for what the
    /// traffic lights stand in for.
    pub(in crate::main_window) fn render_edit_window_control(
        &self,
        item: Item,
        p: &Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if cfg!(target_os = "macos") && item == Item::Fullscreen {
            return Some(self.render_item(item, Place::TitleBar, p, cx));
        }
        caption(item, window.is_maximized(), p)
    }
}

/// The caption button `item` is, off macOS.
fn caption(item: Item, maximized: bool, p: &Palette) -> Option<AnyElement> {
    let caption = match item {
        Item::Minimize => Caption::Minimize,
        Item::Maximize => Caption::Maximize,
        Item::Fullscreen => Caption::Fullscreen,
        Item::Close => Caption::Close,
        _ => return None,
    };
    window_chrome::caption_button(caption, maximized, p)
}

/// How wide the window controls group's `pieces` are.
pub(super) fn controls_width(pieces: &[Piece], cx: &App) -> f32 {
    let width = |piece: &Piece| match piece {
        Piece::Item(item) if item.is_window_control() => {
            if cfg!(target_os = "macos") {
                0.
            } else {
                CAPTION_WIDTH
            }
        }
        piece => items_width(std::slice::from_ref(piece), cx),
    };
    pieces.iter().map(width).sum()
}
