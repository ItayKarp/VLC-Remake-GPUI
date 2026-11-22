//! The List panel (S3.2, `controls-queue.png`): the List button's and Q's panel over the bar's
//! right end, with Queue, Bookmarks and Chapters tabs.
//!
//! The Queue tab lists the queue ([`header`], [`rows`]): rows can be selected, reordered by
//! dragging, played with a double-click or Enter and removed with Delete or "−", and files dropped
//! on it, from the OS or the library (S4.6.7), are inserted where they land ([`editing`]). Bookmarks and Chapters come with their
//! stories (S8.3, S8.2) and say so until then. The header has Repeat and Shuffle (S3.3). The
//! miniplayer's queue (S4.10.4) will reuse the rows. Restored items that are missing or offline
//! are greyed (S3.4).
//!
//! Unlike the track popover and the settings panel, it stays open while the video is used
//! (clicking it, dragging files over it); the List button, Q and Escape close it. It shares their
//! place, so opening one of them closes it, and it keeps the bar up while open.

mod editing;
mod header;
mod rows;
mod selection;
#[cfg(test)]
mod tests;

pub(super) use self::rows::DragPreview;

use std::collections::HashSet;

use futures::StreamExt;
use gpui::{
    AsyncApp, Context, Div, FocusHandle, KeyDownEvent, Stateful, Subscription, Task,
    UniformListScrollHandle, WeakEntity, Window, div, prelude::*, px,
};

use self::selection::Selection;
use super::MainWindow;
use super::bar_panel;
use crate::app::media::Media;
use crate::assets::icons;
use crate::engine::lengths::{self, LengthAnswer, LengthProbe, LengthReceiver};
use crate::queue::Lengths;
use crate::theme::Palette;
use crate::ui::focus;
use crate::ui::menu::ContextMenu;
use crate::ui::popover::BoundsTracker;
use crate::ui::tabs::{Tab, Tabs};

/// The panel's height in the designs.
const DESIGN_HEIGHT: f32 = 945.;
/// The side padding of the tabs' contents, and the muted notes' text size.
const INSET: f32 = 24.;
const NOTE_TEXT: f32 = 14.;
/// How many answers the length probe may deliver before the panel redraws.
const LENGTHS_PER_REDRAW: usize = 64;

/// The panel's tabs, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListTab {
    Queue,
    Bookmarks,
    Chapters,
}

impl ListTab {
    const ALL: [Self; 3] = [Self::Queue, Self::Bookmarks, Self::Chapters];

    fn label(self) -> &'static str {
        match self {
            Self::Queue => "Queue",
            Self::Bookmarks => "Bookmarks",
            Self::Chapters => "Chapters",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Queue => icons::QUEUE,
            Self::Bookmarks => icons::BOOKMARKS,
            Self::Chapters => icons::CHAPTERS,
        }
    }
}

/// Reads queued media's lengths in the background, once the panel first opens.
struct LengthWatch {
    probe: Box<dyn LengthProbe>,
    /// The queue revision last sent to the probe.
    revision: Option<u64>,
    _answers: Task<()>,
}

/// The panel's state: the open tab, its focus, the selection, the lengths and the drag.
pub(super) struct QueuePanel {
    open: Option<ListTab>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    selection: Selection,
    lengths: Lengths,
    watch: Option<LengthWatch>,
    /// The gap a drag over the rows would drop into (0 is above the first row).
    drop_slot: Option<usize>,
    /// The sort menu, while open, and its button.
    sort_menu: Option<(gpui::Entity<ContextMenu>, Subscription)>,
    sort_button: BoundsTracker,
    /// Restored media that wasn't there at launch (S3.4), greyed until it plays.
    pub(super) missing: HashSet<Media>,
}

impl QueuePanel {
    /// A closed panel.
    pub(super) fn new(cx: &mut Context<MainWindow>) -> Self {
        Self {
            open: None,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            selection: Selection::default(),
            lengths: Lengths::default(),
            watch: None,
            drop_slot: None,
            sort_menu: None,
            sort_button: BoundsTracker::new(),
            missing: HashSet::new(),
        }
    }

    /// Whether the panel is open.
    pub(super) fn is_open(&self) -> bool {
        self.open.is_some()
    }
}

impl MainWindow {
    /// Opens the panel on its Queue tab, or closes it (the List button or Q again).
    pub(super) fn toggle_queue_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queue_panel.is_open() {
            self.close_queue_panel(window, cx);
        } else {
            self.open_queue_panel(ListTab::Queue, window, cx);
        }
    }

    /// Opens the panel on `tab`, closing what shares its place, and gives it the keyboard.
    fn open_queue_panel(&mut self, tab: ListTab, window: &mut Window, cx: &mut Context<Self>) {
        self.menu = None;
        self.control_bar.close_overflow();
        self.close_popovers(window, cx);
        self.queue_panel.open = Some(tab);
        self.watch_lengths(cx);
        // After the click that opened it, which focuses the clicked button once its handlers ran.
        let handle = self.queue_panel.focus.clone();
        window.defer(cx, move |window, cx| handle.focus(window, cx));
        cx.notify();
    }

    /// Closes the panel, handing the keyboard back to the player if the panel had it.
    pub(super) fn close_queue_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queue_panel.open.take().is_none() {
            return;
        }
        self.queue_panel.sort_menu = None;
        if self.queue_panel.focus.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    /// Starts the length probe and listens to its answers, redrawing once per batch.
    fn watch_lengths(&mut self, cx: &mut Context<Self>) {
        if self.queue_panel.watch.is_some() {
            return;
        }
        let (sender, receiver) = lengths::channel();
        let answers = cx.spawn(async move |this, cx| receive_lengths(this, receiver, cx).await);
        let probe = lengths::default_probe(sender);
        self.queue_panel.watch = Some(LengthWatch { probe, revision: None, _answers: answers });
    }

    /// Records a batch of the probe's answers, redrawing if any length changed.
    fn take_lengths(&mut self, batch: Vec<LengthAnswer>, cx: &mut Context<Self>) {
        let lengths = &mut self.queue_panel.lengths;
        let changed = batch
            .into_iter()
            .fold(false, |changed, (media, length)| lengths.set(media, length) || changed);
        if changed {
            cx.notify();
        }
    }

    /// Brings the panel up to date with the queue: drops selections of removed entries and asks
    /// for the lengths nobody asked for yet. The player's own length of the current item wins.
    fn sync_queue_panel(&mut self, cx: &Context<Self>) {
        let current = self.queue.current_index().map(|index| &self.queue.entries()[index]);
        let player = self.loaded_player(cx).and_then(|player| player.read(cx).duration());
        if let (Some(entry), Some(duration)) = (current, player) {
            self.queue_panel.lengths.set(entry.media.clone(), Some(duration));
        }
        let panel = &mut self.queue_panel;
        let Some(watch) = &mut panel.watch else { return };
        if watch.revision == Some(self.queue.revision()) {
            return;
        }
        watch.revision = Some(self.queue.revision());
        panel.selection.retain(self.queue.entries());
        watch.probe.clear();
        panel.lengths.retain_queued(self.queue.entries());
        watch.probe.probe(panel.lengths.take_unasked(self.queue.entries()));
    }

    /// The panel over the video's bottom right, while open.
    pub(super) fn render_queue_panel(
        &mut self,
        p: &Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let tab = self.queue_panel.open?;
        self.sync_queue_panel(cx);
        let body = match tab {
            ListTab::Queue => self.render_queue_tab(p, cx),
            ListTab::Bookmarks => note("Bookmarks come with a later version.", p),
            ListTab::Chapters => note("Chapters come with a later version.", p),
        };
        Some(
            bar_panel::surface("queue-panel", DESIGN_HEIGHT, window, p)
                .track_focus(&self.queue_panel.focus)
                .key_context(focus::context(&["QueuePanel", focus::CONTROL]))
                .on_key_down(cx.listener(Self::on_queue_panel_key))
                .child(Self::render_list_tabs(tab, p, cx))
                .child(body),
        )
    }

    fn render_list_tabs(shown: ListTab, p: &Palette, cx: &mut Context<Self>) -> Div {
        let items = ListTab::ALL.iter().map(|tab| Tab::new(Some(tab.icon()), tab.label()));
        let selected = ListTab::ALL.iter().position(|tab| *tab == shown).unwrap_or_default();
        let bar = Tabs::new("list-tabs", items.collect(), selected).on_select(cx.listener(
            |this, index: &usize, window, cx| {
                if let Some(tab) = ListTab::ALL.get(*index) {
                    this.open_queue_panel(*tab, window, cx);
                }
            },
        ));
        div().flex_none().bg(p.well).child(bar)
    }

    fn on_queue_panel_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            cx.stop_propagation();
            self.close_queue_panel(window, cx);
        } else if self.queue_panel.open == Some(ListTab::Queue) && self.queue_key(event, window, cx)
        {
            cx.stop_propagation();
        }
    }
}

/// Passes the probe's answers to the window in batches, until either is gone.
async fn receive_lengths(
    this: WeakEntity<MainWindow>,
    receiver: LengthReceiver,
    cx: &mut AsyncApp,
) {
    let mut batches = receiver.ready_chunks(LENGTHS_PER_REDRAW);
    while let Some(batch) = batches.next().await {
        if this.update(cx, |this, cx| this.take_lengths(batch, cx)).is_err() {
            return;
        }
    }
}

/// A muted line of explanation at the top of a tab.
fn note(text: &'static str, p: &Palette) -> Div {
    div().p(px(INSET)).text_size(px(NOTE_TEXT)).text_color(p.text_muted).child(text)
}
