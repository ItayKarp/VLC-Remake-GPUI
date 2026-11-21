//! The gallery's overlay section: a context menu target, a popover, a dialog, toasts and an
//! error banner.

use gpui::{
    AnyElement, ClickEvent, Context, Div, MouseButton, MouseDownEvent, Window, div, prelude::*, px,
};

use super::basics::{row, section};
use super::{Gallery, updating};
use crate::theme::Palette;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::popover::{self, Side};
use crate::ui::toast::{self, Banner, Level, Toast};

/// A long error, to show how toasts wrap.
const DOWNLOAD_ERROR: &str = "Couldn't download subtitles: the server didn't respond.";
/// A long error with a path, to show how banners wrap.
const SAVE_ERROR: &str = "C:\\Users\\someone\\AppData\\Roaming\\VLC\\settings.toml couldn't be \
                          saved: access denied.";
/// The gap between the popover and its trigger button.
const POPOVER_GAP: f32 = 6.;

impl Gallery {
    /// Triggers for every overlay, and the dismissible banner.
    pub(super) fn overlays(&self, p: &Palette, cx: &Context<Self>) -> Div {
        section("Menus, popovers, dialogs, toasts", p)
            .child(
                row()
                    .child(menu_target(p, cx))
                    .child(self.popover_button(cx))
                    .child(dialog_button(cx))
                    .child(info_toast_button(cx))
                    .child(error_toast_button(cx)),
            )
            .when(self.banner_visible, |s| s.child(save_error_banner(cx)))
    }

    /// The button that opens the popover, with the tracker that records where it is.
    fn popover_button(&self, cx: &Context<Self>) -> Div {
        div().relative().child(self.popover.trigger.element()).child(
            Button::new("popover", "Popover").style(ButtonStyle::Filled).on_click(updating(
                cx,
                |this, _: &ClickEvent| {
                    this.popover.open = !this.popover.open;
                },
            )),
        )
    }

    /// The open popover, placed next to its trigger, or `None` while it is closed.
    pub(super) fn popover_element(
        &self,
        p: &Palette,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !self.popover.open {
            return None;
        }
        let bounds = self.popover.trigger.bounds()?;
        // Like the control bar's popovers: above, flipping below near the top.
        Some(popover::attached(
            bounds,
            Side::Above,
            POPOVER_GAP,
            1,
            self.speed_popover(p, window, cx),
        ))
    }

    fn speed_popover(&self, p: &Palette, window: &Window, cx: &Context<Self>) -> impl IntoElement {
        popover::surface(p, window)
            .id("popover-surface")
            .w(px(280.))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
            .on_mouse_down_out(cx.listener(Self::close_popover_outside))
            .child(div().text_color(p.text_strong).child("Playback speed"))
            .child(self.widgets.sliders.speed.clone())
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(p.text_muted)
                    .child("Opens above its button, and below it when there's no room."),
            )
    }

    /// Closes the popover on a click outside it, except on its own trigger, which toggles it.
    fn close_popover_outside(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.popover.trigger.contains(event.position) {
            self.popover.open = false;
            cx.notify();
        }
    }
}

fn menu_target(p: &Palette, cx: &Context<Gallery>) -> impl IntoElement {
    div()
        .id("menu-target")
        .px(px(16.))
        .py(px(10.))
        .rounded(px(6.))
        .border_1()
        .border_dashed()
        .border_color(p.key_border)
        .child("Right-click for a context menu")
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                this.open_menu(event.position, window, cx);
            }),
        )
}

fn dialog_button(cx: &Context<Gallery>) -> Button {
    Button::new("dialog", "Open dialog…")
        .style(ButtonStyle::Filled)
        .on_click(cx.listener(|this, _, window, cx| this.open_playlist_dialog(window, cx)))
}

fn info_toast_button(cx: &Context<Gallery>) -> Button {
    Button::new("toast-info", "Toast").style(ButtonStyle::Filled).on_click(cx.listener(
        |this, _, _, cx| this.push_toast(Toast::new(Level::Info, "Snapshot saved to Pictures"), cx),
    ))
}

fn error_toast_button(cx: &Context<Gallery>) -> Button {
    Button::new("toast-error", "Error toast").style(ButtonStyle::Filled).on_click(cx.listener(
        |this, _, _, cx| {
            this.push_toast(
                Toast::new(Level::Error, DOWNLOAD_ERROR).action("Retry", |_, _| {}),
                cx,
            );
        },
    ))
}

fn save_error_banner(cx: &Context<Gallery>) -> Banner {
    let gallery = cx.entity().downgrade();
    toast::banner("banner", Level::Error, SAVE_ERROR).action("Retry", |_, _| {}).on_dismiss(
        move |_, cx| {
            gallery
                .update(cx, |this, cx| {
                    this.banner_visible = false;
                    cx.notify();
                })
                .ok();
        },
    )
}
