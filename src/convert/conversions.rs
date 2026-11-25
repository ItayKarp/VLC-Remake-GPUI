//! The app's conversions (S8.10.2): one queue for the whole app, so it outlives the dialog and
//! any window. Each conversion runs on a thread of its own and reports here; the next waiting
//! one starts as one ends. When a batch is over, a system notification says what it came to,
//! and clicking it shows the file it made.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::StreamExt as _;
use futures::channel::oneshot;
use gpui::{
    App, AppContext as _, AsyncApp, Context, Entity, Global, SharedString, SystemNotification,
    Task, WeakEntity,
};

use super::output::Destination;
use super::preset::Preset;
use super::queue::{JobId, Queue, Summary};
use crate::app::media::Media;
use crate::engine::convert::{ConversionEvent, ConversionJob, ConversionReceiver, Request};

/// The notification's tag, so each one replaces the last and its click comes back here.
pub const CONVERSIONS_TAG: &str = "conversions";
/// How long quitting waits for cancelled conversions to remove their files.
const WIND_DOWN: Duration = Duration::from_secs(5);

/// The app's conversions.
#[derive(Default)]
pub struct Conversions {
    queue: Queue,
    /// The running conversions' threads.
    running: HashMap<JobId, ConversionJob>,
    /// Who waits for every conversion to have stopped.
    waiting: Vec<oneshot::Sender<()>>,
    /// The file the last batch made, when it made one, which its notification shows.
    last_output: Option<PathBuf>,
    /// The format and destination last asked for, which the dialog starts with.
    pub choice: (Preset, Destination),
}

/// The app's [`Conversions`].
struct Handle(Entity<Conversions>);

impl Global for Handle {}

/// Starts the app's conversions, cancelled if the app quits under them.
pub fn init(cx: &mut App) {
    let conversions = cx.new(|_| Conversions::default());
    cx.set_global(Handle(conversions));
    cx.on_app_quit(|cx| {
        if let Some(conversions) = Conversions::global(cx) {
            conversions.update(cx, Conversions::cancel_all);
        }
        async {}
    })
    .detach();
}

impl Conversions {
    /// The app's conversions, once [`init`] ran (tests may not start them).
    pub fn global(cx: &App) -> Option<Entity<Self>> {
        cx.try_global::<Handle>().map(|handle| handle.0.clone())
    }

    /// The conversions asked for this session.
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// The queue itself, so a test puts conversions in any state without running one.
    #[cfg(test)]
    pub fn queue_mut(&mut self) -> &mut Queue {
        &mut self.queue
    }

    /// Queues converting each of `sources` to `preset` under `destination`, and starts what may
    /// start.
    pub fn add(
        &mut self,
        sources: Vec<Media>,
        preset: Preset,
        destination: Destination,
        cx: &mut Context<Self>,
    ) {
        for source in sources {
            self.queue.add(source, preset, &destination, Path::exists);
        }
        self.choice = (preset, destination);
        self.start_due(cx);
        cx.notify();
    }

    /// Cancels conversion `id`, removing what it wrote.
    pub fn cancel(&mut self, id: JobId, cx: &mut Context<Self>) {
        if self.queue.cancel(id)
            && let Some(job) = self.running.get(&id)
        {
            job.cancel();
        }
        self.start_due(cx);
        self.summarize(cx);
        cx.notify();
    }

    /// Cancels every conversion not over yet.
    pub fn cancel_all(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<JobId> = self.queue.jobs().iter().map(|job| job.id).collect();
        for id in ids {
            if self.queue.cancel(id)
                && let Some(job) = self.running.get(&id)
            {
                job.cancel();
            }
        }
        self.summarize(cx);
        cx.notify();
    }

    /// Forgets the conversions that are over.
    pub fn clear_finished(&mut self, cx: &mut Context<Self>) {
        self.queue.clear_finished();
        cx.notify();
    }

    /// Resolves once no conversion's thread runs, so their files are finished or removed, or
    /// after [`WIND_DOWN`].
    pub fn stopped(&mut self, cx: &mut Context<Self>) -> Task<()> {
        let (sender, receiver) = oneshot::channel();
        if self.running.is_empty() {
            _ = sender.send(());
        } else {
            self.waiting.push(sender);
        }
        let timeout = cx.background_executor().timer(WIND_DOWN);
        cx.background_spawn(async move {
            futures::future::select(receiver, Box::pin(timeout)).await;
        })
    }

    /// Shows the file the last batch made in the OS's file manager.
    pub fn reveal_last(&self, cx: &App) {
        if let Some(output) = &self.last_output {
            cx.reveal_path(output);
        }
    }

    /// Starts the waiting conversions that may start now.
    fn start_due(&mut self, cx: &mut Context<Self>) {
        for job in self.queue.start_due() {
            let request = Request {
                options: job.preset.media_options(&job.output),
                audio_only: job.preset.is_audio_only(),
                source: job.source,
                output: job.output,
            };
            let (thread, events) = ConversionJob::start(request);
            let id = job.id;
            self.running.insert(id, thread);
            cx.spawn(async move |this, cx| pump(this, id, events, cx).await).detach();
        }
    }

    /// Conversion `id` reported `event`.
    fn on_event(&mut self, id: JobId, event: ConversionEvent, cx: &mut Context<Self>) {
        match event {
            ConversionEvent::Progress(fraction) => self.queue.set_progress(id, fraction),
            ConversionEvent::Finished(result) => {
                self.queue.finish(id, result.map_err(|failure| failure.message()));
                self.ended(id, cx);
            }
            ConversionEvent::Cancelled => self.ended(id, cx),
        }
        cx.notify();
    }

    /// Conversion `id`'s thread is done: the next one starts, and those waiting for every
    /// conversion to stop hear once none runs.
    fn ended(&mut self, id: JobId, cx: &mut Context<Self>) {
        self.running.remove(&id);
        self.start_due(cx);
        if self.running.is_empty() {
            for waiting in self.waiting.drain(..) {
                _ = waiting.send(());
            }
        }
        self.summarize(cx);
    }

    /// Once a batch is over, says what it came to.
    fn summarize(&mut self, cx: &App) {
        let Some(summary) = self.queue.take_summary() else { return };
        self.last_output = match summary.done.as_slice() {
            [only] => Some(only.clone()),
            _ => None,
        };
        let (title, body) = notification_text(&summary);
        cx.show_system_notification(SystemNotification {
            tag: CONVERSIONS_TAG.into(),
            title: title.into(),
            body: SharedString::from(body),
            actions: Vec::new(),
        });
    }
}

/// Hands conversion `id`'s reports to `conversions` until it ends.
async fn pump(
    conversions: WeakEntity<Conversions>,
    id: JobId,
    mut events: ConversionReceiver,
    cx: &mut AsyncApp,
) {
    while let Some(event) = events.next().await {
        if conversions.update(cx, |this, cx| this.on_event(id, event, cx)).is_err() {
            return;
        }
    }
}

/// The notification's title and text for a batch that came to `summary`.
fn notification_text(summary: &Summary) -> (String, String) {
    let file_name = |path: &PathBuf| {
        path.file_name().map_or_default(|name| name.to_string_lossy().into_owned())
    };
    let failed = match summary.failed.as_slice() {
        [] => None,
        [why] if summary.done.is_empty() => Some(format!("{why}.")),
        many => Some(trn!(
            "{count} file couldn't be converted.",
            "{count} files couldn't be converted.",
            many.len()
        )),
    };
    let done = match summary.done.as_slice() {
        [] => None,
        [only] => Some(tr!("Saved {name}.", name = file_name(only))),
        many => Some(trn!("Converted {count} file.", "Converted {count} files.", many.len())),
    };
    let title = if done.is_some() { tr!("Conversion finished") } else { tr!("Conversion failed") };
    let body: Vec<String> = done.into_iter().chain(failed).collect();
    (title.to_owned(), body.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(done: &[&str], failed: &[&str]) -> Summary {
        Summary {
            done: done.iter().map(PathBuf::from).collect(),
            failed: failed.iter().map(|why| (*why).to_owned()).collect(),
        }
    }

    #[test]
    fn one_file_is_named() {
        let (title, body) = notification_text(&summary(&["/out/film.mp4"], &[]));
        assert_eq!((title.as_str(), body.as_str()), ("Conversion finished", "Saved film.mp4."));
    }

    #[test]
    fn many_files_are_counted_with_the_failures() {
        let (_, body) = notification_text(&summary(&["/a.mp4", "/b.mp4"], &["x", "y"]));
        assert_eq!(body, "Converted 2 files. 2 files couldn't be converted.");
    }

    #[test]
    fn a_lone_failure_says_why() {
        let (title, body) = notification_text(&summary(&[], &["The disk is full"]));
        assert_eq!((title.as_str(), body.as_str()), ("Conversion failed", "The disk is full."));
    }
}
