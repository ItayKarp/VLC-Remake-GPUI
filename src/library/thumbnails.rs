//! The thumbnail service (S4.4): the GPUI global the views ask for thumbnails through. It starts
//! the [`thumbnailer`](super::thumbnailer) thread and keeps its answers for the media the views
//! asked about; windows observe the global to show pictures as they arrive.
//!
//! A view calls [`show`] with the media on screen whenever that changes, and draws each with
//! [`status`]: the picture when it is [`Status::Ready`], a placeholder otherwise
//! (`ui::thumbnail`). Players report whether they play ([`set_playing`]), and the thumbnailer
//! holds its background work back meanwhile.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{App, BorrowAppContext as _, EntityId, Global};

use super::artwork_cache::ArtworkCache;
use super::indexer::store::Store;
use super::thumbnail_cache::ThumbnailCache;
use super::thumbnailer::{self, Answer, Command, Thumbnail};
use crate::db::media::MediaId;
use crate::engine::thumbnail;
use crate::opening::Cancel;
use crate::settings;

/// The thumbnail cache's folder in the cache dir.
const THUMBNAILS_DIR_NAME: &str = "thumbnails";

/// The thumbnail service: the thread's channel and its answers so far.
pub struct Thumbnails {
    commands: Sender<Command>,
    /// The answers about media the views asked for.
    known: HashMap<MediaId, Option<Thumbnail>>,
    /// Asked for and not answered yet.
    waiting: HashSet<MediaId>,
    /// The players that play now.
    playing: HashSet<EntityId>,
}

impl Global for Thumbnails {}

/// What a view should draw for a media.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status<'a> {
    /// Its pictures.
    Ready(&'a Thumbnail),
    /// It has none: draw the placeholder for good.
    Missing,
    /// Not known yet: draw the placeholder until it is.
    Pending,
}

/// Starts the thumbnailer on `store`, with covers from `artwork_dir`. Nothing is cached without
/// a cache dir, so there is no service then.
pub fn init(store: Box<dyn Store>, covers: Option<ArtworkCache>, cancel: Cancel, cx: &mut App) {
    let Some(dir) = settings::cache_dir() else { return };
    let cache = ThumbnailCache::new(dir.join(THUMBNAILS_DIR_NAME));
    let maker = thumbnailer::make::Maker::new(thumbnail::default_grabber(), cache, covers);
    let (answers, mut received) = mpsc::unbounded();
    let report = Box::new(move |answer| drop(answers.unbounded_send(answer)));
    let commands = thumbnailer::spawn(store, maker, report, cancel);
    cx.set_global(Thumbnails {
        commands,
        known: HashMap::new(),
        waiting: HashSet::new(),
        playing: HashSet::new(),
    });
    cx.spawn(async move |cx| {
        while let Some(answer) = received.next().await {
            cx.update(|cx| cx.update_global::<Thumbnails, _>(|service, _| service.learn(answer)));
        }
    })
    .detach();
}

/// Tells the thumbnailer which media are on screen, in order: those it hasn't answered about
/// come first, and work for media no longer on screen is dropped (S4.4.4).
#[allow(dead_code, reason = "the library views (S4.6) are its first callers")]
pub fn show(media: &[MediaId], cx: &mut App) {
    if !cx.has_global::<Thumbnails>() {
        return;
    }
    cx.update_global::<Thumbnails, _>(|service, _| {
        let unknown: Vec<MediaId> =
            media.iter().copied().filter(|id| !service.known.contains_key(id)).collect();
        service.waiting = unknown.iter().copied().collect();
        service.send(Command::Show(unknown));
    });
}

/// What to draw for `media`.
#[allow(dead_code, reason = "the library views (S4.6) are its first callers")]
pub fn status(media: MediaId, cx: &App) -> Status<'_> {
    let known = cx.try_global::<Thumbnails>().and_then(|service| service.known.get(&media));
    match known {
        Some(Some(thumbnail)) => Status::Ready(thumbnail),
        Some(None) => Status::Missing,
        None => Status::Pending,
    }
}

/// Notes whether `player` plays; the thumbnailer's background work waits while any does.
pub fn set_playing(player: EntityId, playing: bool, cx: &mut App) {
    if !cx.has_global::<Thumbnails>() {
        return;
    }
    cx.update_global::<Thumbnails, _>(|service, _| {
        let was = !service.playing.is_empty();
        if playing {
            service.playing.insert(player);
        } else {
            service.playing.remove(&player);
        }
        let is = !service.playing.is_empty();
        if was != is {
            service.send(Command::Playing(is));
        }
    });
}

impl Thumbnails {
    /// Keeps `answer` if a view asked about its media (background answers for media no view
    /// asked about would only take memory).
    fn learn(&mut self, answer: Answer) {
        if self.waiting.remove(&answer.media) || self.known.contains_key(&answer.media) {
            self.known.insert(answer.media, answer.thumbnail);
        }
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::warn!("library: the thumbnailer is gone");
        }
    }
}
