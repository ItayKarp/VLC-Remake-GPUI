//! Helpers for the window tests of the dialogs drawn in the shared frame (Stream, Open Capture
//! Device, Open Disc, Convert) and of the main window's side of them: a dialog alone in a test
//! window, a frame drawn as the window would, and a silent mock player in the main window.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{App, Context, Entity, Render, TestAppContext, VisualTestContext, Window};

use crate::app::media::Media;
use crate::engine::mock::{MockHandle, MockMedia};
use crate::engine::player::Player;
use crate::main_window::now_playing::tests::play_media;

/// Sets up what every dialog draws with: settings in memory, the theme and the UI kit.
fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        crate::settings::init_in_memory(cx);
        crate::theme::init(cx);
        crate::ui::init(cx);
    });
}

/// Opens the view `build` makes alone in a test window and draws it.
pub(in crate::main_window) fn open_view<V: Render + 'static>(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
) -> (Entity<V>, &mut VisualTestContext) {
    init(cx);
    let (view, cx) = cx.add_window_view(build);
    draw(cx);
    (view, cx)
}

/// Settles the tasks and paints a frame, as the window would.
pub(in crate::main_window) fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

/// What a dialog handed over once it closed, in order.
pub(in crate::main_window) type Handed<T> = Rc<RefCell<Vec<T>>>;

/// A callback recording what it is handed, and the record.
pub(in crate::main_window) fn recorder<T: 'static>()
-> (Handed<T>, impl Fn(T, &mut Window, &mut App) + 'static) {
    let handed: Handed<T> = Rc::default();
    let log = handed.clone();
    (handed, move |item, _: &mut Window, _: &mut App| log.borrow_mut().push(item))
}

/// Gives the main window a player on the mock backend, which plays nothing and is silent, and
/// plays a mock clip in it, wired to the window as a real player is.
pub(in crate::main_window) fn play_mock(
    cx: &mut VisualTestContext,
) -> (Entity<Player>, MockHandle) {
    let playing = play_media(cx, MockMedia::default(), mock_clip());
    draw(cx);
    (playing.player, playing.mock)
}

/// The clip the mock player opens.
pub(in crate::main_window) fn mock_clip() -> Media {
    Media::Url("https://e.com/clip".into())
}
