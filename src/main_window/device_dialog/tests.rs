//! Helpers for the window tests of the dialogs drawn in the shared frame (Stream, Open Capture
//! Device, Open Disc, Convert) and of the main window's side of them: a dialog alone in a test
//! window, a frame drawn as the window would, and a silent mock player in the main window.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, Entity, Render, Subscription, TestAppContext, VisualTestContext,
    Window,
};

use crate::app::media::Media;
use crate::engine::backend::Backend;
use crate::engine::mock::{MockBackend, MockHandle, MockMedia};
use crate::engine::player::{Player, PlayerEvent};
use crate::main_window::MainWindow;
use crate::main_window::tests::main_view;

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

/// Gives the main window a player on the mock backend, which plays nothing and is silent,
/// with `on_event` hearing its events, and opens a mock clip in it.
pub(in crate::main_window) fn play_mock(
    cx: &mut VisualTestContext,
    on_event: impl Fn(&mut MainWindow, &PlayerEvent, &mut Context<MainWindow>) + 'static,
) -> (Entity<Player>, MockHandle) {
    let main = main_view(cx);
    let slot: Rc<RefCell<Option<MockHandle>>> = Rc::default();
    let out = slot.clone();
    let player = main.update_in(cx, |main, window, cx| {
        let player = cx.new(|cx| {
            let backend = move |events| {
                let (mock, handle) = MockBackend::new(events, MockMedia::default());
                *out.borrow_mut() = Some(handle);
                Box::new(mock) as Box<dyn Backend>
            };
            Player::new(backend, cx)
        });
        let events: Subscription =
            cx.subscribe_in(&player, window, move |main, _, event, _, cx| {
                on_event(main, event, cx);
            });
        main.player = Some((player.clone(), events));
        player
    });
    player.update(cx, |player, cx| player.open(mock_clip(), cx));
    draw(cx);
    let handle = slot.borrow_mut().take().expect("the player made its backend");
    (player, handle)
}

/// The clip the mock player opens.
pub(in crate::main_window) fn mock_clip() -> Media {
    Media::Url("mock://clip".into())
}
