//! Handling launches that arrive while the app runs (S0.6.1, S0.6.3): files from a
//! second launch, and macOS "open these files" events.
//!
//! Launches are batched ([`batch`]), then go to the main window. While onboarding
//! is showing they wait in [`PendingOpen`] and open when it hands off.

use std::time::Instant;

use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use gpui::{App, AsyncApp, Global};

use super::instance::batch::{BATCH_WINDOW, Batcher};
use super::instance::{InboxReceiver, Incoming};
use super::request::{Launch, OpenRequest, StartScreen};
use crate::main_window::MainWindow;
use crate::onboarding::Onboarding;

/// Files asked for while onboarding was showing, opened when it finishes.
#[derive(Default)]
struct PendingOpen(Option<OpenRequest>);

impl Global for PendingOpen {}

/// Drains the inbox on the UI thread for as long as the app runs.
pub fn receive(inbox: InboxReceiver, cx: &mut App) {
    inbox.mark_running();
    let mut receiver = inbox.receiver;
    cx.spawn(async move |cx: &mut AsyncApp| {
        let mut batcher = Batcher::new(Instant::now());
        while let Some(first) = receiver.next().await {
            let first_arrival = Instant::now();
            let batch = collect_batch(first, &mut receiver, cx).await;
            let launch = batcher.combine(batch, first_arrival, Instant::now());
            cx.update(|cx| deliver(launch, cx));
        }
    })
    .detach();
}

/// Waits until launches stop arriving, so a multi-file selection opens as one.
async fn collect_batch(
    first: Incoming,
    receiver: &mut UnboundedReceiver<Incoming>,
    cx: &AsyncApp,
) -> Vec<Launch> {
    let mut batch = vec![take(first)];
    loop {
        cx.background_executor().timer(BATCH_WINDOW).await;
        let before = batch.len();
        batch.extend(std::iter::from_fn(|| receiver.try_recv().ok()).map(take));
        if batch.len() == before {
            return batch;
        }
    }
}

/// Acknowledges a launch (the sender may exit now) and keeps what it asked for.
fn take(mut incoming: Incoming) -> Launch {
    incoming.acknowledge();
    incoming.launch
}

/// Does what a launch asked for in this running app.
fn deliver(launch: Launch, cx: &mut App) {
    log::debug!("opening forwarded launch {launch:?}");
    match launch.start {
        StartScreen::Onboarding => super::run_setup_again(super::preferred_display(cx), cx),
        StartScreen::Gallery => super::open_initial_window(&launch, cx),
        StartScreen::Auto => open(launch.open, cx),
    }
}

/// Opens `request` in the main window, or queues it behind onboarding. With no
/// window at all (macOS keeps running without one), opens the first window.
fn open(request: OpenRequest, cx: &mut App) {
    let windows = cx.windows();
    if let Some(main) = windows.iter().find_map(gpui::AnyWindowHandle::downcast::<MainWindow>) {
        main.update(cx, |main, window, cx| {
            main.open(request, window, cx);
            window.activate_window();
        })
        .ok();
    } else if let Some(onboarding) =
        windows.iter().find_map(gpui::AnyWindowHandle::downcast::<Onboarding>)
    {
        queue(request, cx);
        onboarding.update(cx, |_, window, _| window.activate_window()).ok();
    } else {
        super::open_initial_window(&Launch { open: request, start: StartScreen::Auto }, cx);
    }
    cx.activate(true);
}

/// Keeps `request` until onboarding hands off to the main window.
fn queue(request: OpenRequest, cx: &mut App) {
    if request.is_empty() {
        return;
    }
    let pending = cx.default_global::<PendingOpen>();
    pending.0 = Some(match pending.0.take() {
        Some(earlier) => earlier.then(request),
        None => request,
    });
}

/// What was asked for while onboarding was showing, if anything. Empties the queue.
pub fn take_pending(cx: &mut App) -> OpenRequest {
    cx.default_global::<PendingOpen>().0.take().unwrap_or_default()
}
