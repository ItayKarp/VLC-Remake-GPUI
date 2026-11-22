//! The thumbnailer thread: takes the next job from the [`Queue`], answers from the cache when it
//! can, makes the pictures when it must, and keeps the cache under its size cap.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use super::make::{Made, Maker};
use super::queue::{Job, Priority, Queue};
use super::{Answer, Command, Report, Thumbnail};
use crate::db::media::MediaId;
use crate::db::metadata::MediaKind;
use crate::db::now_ms;
use crate::db::thumbnails::{self, Cached, Subject};
use crate::library::indexer::store::{Store, call};
use crate::library::thumbnail_cache::MAX_BYTES;
use crate::opening::Cancel;

/// The pause after each picture made in the background, so filling the cache never takes a
/// whole core for long.
const THROTTLE: Duration = Duration::from_millis(250);
/// How often an empty backlog is looked for new videos: the indexer reads them meanwhile, and
/// a first index should show thumbnails soon after.
const REFILL_EVERY: Duration = Duration::from_secs(15);
/// How many videos the backlog takes at a time.
const BACKLOG_BATCH: usize = 32;
/// How much is saved between eviction checks.
const EVICT_AFTER_BYTES: u64 = 8 * 1024 * 1024;

/// Starts the thumbnailer thread on `store`, making pictures with `maker` and answering through
/// `report`. It stops once `cancel` is set or every sender is gone.
pub fn spawn(
    store: Box<dyn Store>,
    maker: Maker,
    report: Report,
    cancel: Cancel,
) -> Sender<Command> {
    let (sender, commands) = mpsc::channel();
    let started = std::thread::Builder::new().name("thumbnails".into()).spawn(move || {
        let worker = Worker {
            store,
            maker,
            report,
            cancel,
            commands,
            queue: Queue::default(),
            saved: 0,
            next_fill: Instant::now(),
        };
        worker.run();
    });
    if let Err(error) = started {
        tracing::error!("library: couldn't start the thumbnailer: {error}");
    }
    sender
}

struct Worker {
    store: Box<dyn Store>,
    maker: Maker,
    report: Report,
    cancel: Cancel,
    commands: Receiver<Command>,
    queue: Queue,
    /// Bytes saved since the cache was last checked against its cap.
    saved: u64,
    /// When an empty backlog is next looked at.
    next_fill: Instant,
}

impl Worker {
    fn run(mut self) {
        self.evict();
        while !self.cancel.is_cancelled() {
            take_waiting(&self.commands, &mut self.queue);
            let going = if let Some(job) = self.queue.next() {
                self.run_job(job);
                job.priority == Priority::Visible || self.wait(THROTTLE)
            } else {
                let wait = self.next_fill.saturating_duration_since(Instant::now());
                self.fill() || self.wait(wait.max(THROTTLE))
            };
            if !going {
                return;
            }
        }
    }

    /// Waits up to `timeout` for a command; `false` once every sender is gone.
    fn wait(&mut self, timeout: Duration) -> bool {
        match self.commands.recv_timeout(timeout) {
            Ok(command) => apply(command, &mut self.queue),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return false,
        }
        true
    }

    /// Takes the next videos into the backlog, if it may run; `true` when it found some.
    fn fill(&mut self) -> bool {
        let now = Instant::now();
        if !self.queue.may_fill() || !self.maker.can_grab() || now < self.next_fill {
            return false;
        }
        let found =
            call(&*self.store, |c| thumbnails::backlog(c, BACKLOG_BATCH)).unwrap_or_default();
        if found.is_empty() {
            self.next_fill = now + REFILL_EVERY;
            return false;
        }
        self.queue.fill(found);
        true
    }

    fn run_job(&mut self, job: Job) {
        let id = job.media;
        let Some(subject) = call(&*self.store, move |c| thumbnails::subject(c, id)).flatten()
        else {
            self.answer(job, None);
            return;
        };
        if job.priority == Priority::Visible && self.answer_cached(id) {
            return;
        }
        if subject.gave_up {
            self.answer(job, None);
            return;
        }
        match self.make(job, &subject) {
            Made::Pictures(images) => self.keep(job, &images),
            Made::Nothing => self.answer(job, None),
            Made::Stopped if subject.kind == MediaKind::Video => {
                call(&*self.store, move |c| thumbnails::try_again(c, id));
            }
            Made::Stopped => {}
        }
    }

    /// Answers with what the cache already has for `id`; `false` when it has nothing whole.
    fn answer_cached(&mut self, id: MediaId) -> bool {
        let images = call(&*self.store, move |c| thumbnails::cached(c, id)).unwrap_or_default();
        if images.is_empty() {
            return false;
        }
        if !self.maker.cache().has_all(&images) {
            // Evicted or deleted on the disk: make them again.
            call(&*self.store, move |c| thumbnails::forget(c, id));
            return false;
        }
        let now = now_ms();
        call(&*self.store, move |c| thumbnails::touch(c, id, now));
        let thumbnail = Thumbnail::new(self.maker.cache(), &images);
        (self.report)(Answer { media: id, thumbnail: Some(thumbnail) });
        true
    }

    fn make(&mut self, job: Job, subject: &Subject) -> Made {
        match (subject.kind, &subject.path) {
            (MediaKind::Audio, _) => self.maker.cover(subject.artwork.as_deref()),
            (MediaKind::Video, Some(path)) => {
                let id = subject.id;
                // Marked first: a file that crashes the app isn't tried on every launch.
                let now = now_ms();
                call(&*self.store, move |c| thumbnails::give_up(c, id, now));
                let (cancel, commands, queue) = (&self.cancel, &self.commands, &mut self.queue);
                let mut wanted = || {
                    take_waiting(commands, queue);
                    let on_screen = job.priority == Priority::Background || queue.is_on_screen(id);
                    on_screen && !cancel.is_cancelled()
                };
                self.maker.video(&subject.identity, path, subject.duration, &mut wanted)
            }
            // Not read yet, or not on the disk: the views ask again later.
            (MediaKind::Video | MediaKind::Unknown, _) => Made::Stopped,
        }
    }

    /// Records `images` as `job`'s media's pictures and answers with them.
    fn keep(&mut self, job: Job, images: &[Cached]) {
        let (id, now, rows) = (job.media, now_ms(), images.to_vec());
        call(&*self.store, move |c| thumbnails::record(c, id, &rows, now));
        self.saved += images.iter().map(|image| image.bytes).sum::<u64>();
        if self.saved >= EVICT_AFTER_BYTES {
            self.evict();
        }
        self.answer(job, Some(Thumbnail::new(self.maker.cache(), images)));
    }

    fn answer(&self, job: Job, thumbnail: Option<Thumbnail>) {
        (self.report)(Answer { media: job.media, thumbnail });
    }

    fn evict(&mut self) {
        self.saved = 0;
        let cache = self.maker.cache().clone();
        call(&*self.store, move |c| cache.evict(c, MAX_BYTES));
    }
}

/// Applies the commands already waiting, without blocking.
fn take_waiting(commands: &Receiver<Command>, queue: &mut Queue) {
    while let Ok(command) = commands.try_recv() {
        apply(command, queue);
    }
}

fn apply(command: Command, queue: &mut Queue) {
    match command {
        Command::Show(media) => queue.show(media),
        Command::Playing(playing) => queue.set_playing(playing),
    }
}
