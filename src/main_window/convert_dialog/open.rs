//! Convert in the main window: the menu command (Ctrl+R) opens the dialog holding what plays,
//! the library's file menu opens it holding the files picked, and quitting or closing the window
//! while conversions run asks whether to cancel them (S8.10 edge case).

use gpui::{App, AppContext as _, Context, Div, Window, prelude::*};

use super::ConvertDialog;
use crate::app::media::Media;
use crate::convert::Conversions;
use crate::keymap::actions::Convert;
use crate::main_window::MainWindow;
use crate::ui::modal::{Dialog, DialogButton};

impl MainWindow {
    /// Adds the handler for Convert.
    pub(in crate::main_window) fn convert_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &Convert, window, cx| {
            let playing = this.playing_media(cx).into_iter().collect();
            this.show_convert(playing, window, cx);
        }))
    }

    /// Opens the Convert dialog holding `sources`.
    pub(in crate::main_window) fn show_convert(
        &mut self,
        sources: Vec<Media>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(conversions) = Conversions::global(cx) else { return };
        let dialog = cx.new(|cx| ConvertDialog::new(sources, conversions, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// What the player has open, if anything.
    fn playing_media(&self, cx: &App) -> Option<Media> {
        let (player, _) = self.player.as_ref()?;
        player.read(cx).media().cloned()
    }

    /// Asks whether to cancel the `count` unfinished conversions and quit.
    fn ask_to_cancel_conversions(
        &mut self,
        count: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let quit =
            DialogButton::new("Cancel and Quit").primary().on_click(|_, cx| cancel_and_quit(cx));
        let buttons = vec![DialogButton::new("Keep Converting"), quit];
        let body = move |_: &mut Window, _: &mut App| {
            let what = if count == 1 { "A conversion hasn't" } else { "Some conversions haven't" };
            format!("{what} finished ({count} to go). Quitting cancels and deletes the unfinished files.")
                .into_any_element()
        };
        let ask = cx.new(|cx| Dialog::new("Cancel conversions and quit?", body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(ask, window, cx));
    }
}

/// Quits, unless conversions are running: then a main window asks first, and the app quits once
/// the user agrees and the conversions have removed their files.
pub fn ask_before_quitting(cx: &mut App) {
    let count = unfinished(cx);
    if count == 0 {
        cx.quit();
        return;
    }
    let main = cx.windows().into_iter().find_map(|window| window.downcast::<MainWindow>());
    let Some(main) = main else {
        cancel_and_quit(cx);
        return;
    };
    main.update(cx, |main, window, cx| {
        window.activate_window();
        main.ask_to_cancel_conversions(count, window, cx);
    })
    .ok();
}

/// Whether closing the main window may go ahead now: not while conversions run, which it asks
/// about instead (closing the last window quits).
pub(in crate::main_window) fn may_close(
    main: &gpui::WeakEntity<MainWindow>,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let count = unfinished(cx);
    if count == 0 {
        return true;
    }
    main.update(cx, |main, cx| main.ask_to_cancel_conversions(count, window, cx)).ok();
    false
}

/// How many conversions are waiting or running.
fn unfinished(cx: &App) -> usize {
    Conversions::global(cx).map_or(0, |conversions| conversions.read(cx).queue().unfinished())
}

/// Cancels every conversion and quits once they have stopped.
fn cancel_and_quit(cx: &mut App) {
    let Some(conversions) = Conversions::global(cx) else {
        cx.quit();
        return;
    };
    let stopped = conversions.update(cx, |conversions, cx| {
        conversions.cancel_all(cx);
        conversions.stopped(cx)
    });
    cx.spawn(async move |cx| {
        stopped.await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}
