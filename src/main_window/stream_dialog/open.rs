//! Stream in the main window: the menu command (Ctrl+S) opens the dialog for what plays, the
//! library's file menu for a file. Starting streams the media playing from where it is, or plays
//! the file next in the queue, streaming; a toast says where to watch. When the stream couldn't
//! start, or ends with its media, the main window says so.

#[cfg(test)]
mod tests;

use gpui::{App, AppContext as _, ClipboardItem, Context, Div, Window, prelude::*};

use super::StreamDialog;
use crate::app::media::Media;
use crate::engine::player::PlayerEvent;
use crate::engine::stream_out::StreamTarget;
use crate::engine::values::Start;
use crate::keymap::actions::Stream;
use crate::main_window::MainWindow;
use crate::ui::toast::{Level, Toast};

/// What Stream says with nothing playing.
const NOTHING_PLAYING: &str =
    tr_noop!("Play something to stream it, or pick Stream in a file's menu in the library.");
/// What a stream that couldn't start says.
const FAILED: &str = tr_noop!(
    "The stream couldn't start, so the media plays here without it. Another \
     program may have taken its port, or its address can't be reached."
);

impl MainWindow {
    /// Adds the handler for Stream.
    pub(in crate::main_window) fn stream_actions(root: Div, cx: &mut Context<Self>) -> Div {
        root.on_action(cx.listener(|this, _: &Stream, window, cx| {
            match this.loaded_player(cx).and_then(|player| player.read(cx).media().cloned()) {
                Some(media) => this.show_stream(media, window, cx),
                None => {
                    this.show_toast(
                        Toast::new(Level::Info, crate::i18n::text(NOTHING_PLAYING)),
                        cx,
                    );
                }
            }
        }))
    }

    /// Opens the Stream dialog for `media`, showing its stream if it streams now.
    pub(in crate::main_window) fn show_stream(
        &mut self,
        media: Media,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let running = self.running_stream(&media, cx);
        let main = cx.entity().downgrade();
        let chosen = media.clone();
        let on_choose = move |target: Option<StreamTarget>, window: &mut Window, cx: &mut App| {
            let media = chosen.clone();
            main.update(cx, |main, cx| main.choose_stream(media, target, window, cx)).ok();
        };
        let dialog = cx.new(|cx| StreamDialog::new(media, running, on_choose, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// The stream `media` goes to now, if it is what's open and streams.
    fn running_stream(&self, media: &Media, cx: &App) -> Option<StreamTarget> {
        let player = self.loaded_player(cx)?;
        let player = player.read(cx);
        player.stream_target().copied().filter(|_| player.media() == Some(media))
    }

    /// Streams `media` to `target`, or stops its stream with `None`.
    fn choose_stream(
        &mut self,
        media: Media,
        target: Option<StreamTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = target else {
            if let Some(player) = self.loaded_player(cx) {
                player.update(cx, |player, cx| player.stream(None, cx));
            }
            self.show_osd(tr!("Stopped streaming"), cx);
            return;
        };
        let open = self.loaded_player(cx).filter(|player| {
            let player = player.read(cx);
            player.media() == Some(&media) && player.state().is_active()
        });
        if let Some(player) = open {
            player.update(cx, |player, cx| player.stream(Some(target), cx));
        } else {
            let index = self.queue.insert_next(vec![media]);
            let start = Start { stream: Some(target), ..Start::default() };
            self.play_from(index, start, window, cx);
        }
        self.show_toast(streaming_toast(&target), cx);
    }

    /// Says when a stream couldn't start.
    pub(in crate::main_window) fn on_stream_event(
        &mut self,
        event: &PlayerEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            PlayerEvent::StreamFailed => {
                self.show_toast(Toast::new(Level::Error, crate::i18n::text(FAILED)), cx);
            }
            PlayerEvent::StreamChanged => cx.notify(),
            _ => {}
        }
    }
}

/// Where to watch `target`, with Copy Link.
fn streaming_toast(target: &StreamTarget) -> Toast {
    let url = target.url();
    let message = tr!("Streaming. Watch it at {url}", url);
    Toast::new(Level::Info, message).lasting_action(tr!("Copy Link"), move |_, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
    })
}
