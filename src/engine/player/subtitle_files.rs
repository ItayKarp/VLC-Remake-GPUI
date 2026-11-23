//! The [`Player`]'s subtitle files (S6.1): the ones found beside a local video as it opens
//! (loaded as tracks without showing one, so auto-selection picks among them by the languages
//! their names give), the ones the user adds (shown), and reading one again in another encoding
//! (S6.1.6); and the ones linked to it in the Add Subtitles dialog (S6.4).
//!
//! Finding files and reading their encodings happens off the UI thread, but files reach the
//! backend in the order they were asked for, after the backend listed the media's own tracks: a
//! file learns its track as the one that appears after it was loaded (`claim_added`), so a file
//! loaded earlier, or out of order, would take another's track for its own.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use encoding_rs::Encoding;
use gpui::{AppContext as _, Context};

use super::{Player, PlayerEvent};
use crate::app::media::Media;
use crate::engine::cues;
use crate::engine::sidecar::{self, Sidecar};
use crate::engine::subtitle_style::SubtitleLoading;
use crate::engine::subtitle_text::{self, EncodingChoice};
use crate::engine::tracks::{SubtitleFile, TrackId, TrackKind, Tracks};
use crate::engine::values::Generation;

/// When subtitle files linked to the media load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linked {
    /// As the media opens: none shows unless auto-selection picks it.
    OnOpen,
    /// The user just linked them: the first shows.
    Now,
}

/// A subtitle file ready to load, waiting for its turn.
#[derive(Clone, Debug)]
pub(super) struct Pending {
    /// The file as the player lists it.
    file: SubtitleFile,
    /// What the backend is given: the file, or its UTF-8 copy.
    load: PathBuf,
    /// Whether it shows once loaded (a file the user added or read again).
    show: bool,
}

impl Pending {
    /// Readies `file` to load, read as `choice` says, and reads its cues when the app can draw
    /// it (S6.2). Reads the file, so it runs off the UI thread.
    fn prepare(
        mut file: SubtitleFile,
        choice: EncodingChoice,
        loading: &SubtitleLoading,
        show: bool,
    ) -> Self {
        let prepared = subtitle_text::prepare(&file.path, choice, loading.converted.as_deref());
        let text = subtitle_text::decode(&prepared, &loading.encoding);
        file.cues = text.and_then(|text| cues::parse(&file.path, &text)).map(Arc::new);
        file.encoding = prepared.encoding;
        Self { file, load: prepared.path, show }
    }
}

impl Player {
    /// Looks for subtitle files beside the media just opened, when it is a local file and
    /// "Load subtitle files automatically" is on, and loads what it finds.
    pub(super) fn find_sidecars(&mut self, cx: &mut Context<Self>) {
        self.subtitle_loads = None;
        let loading = self.media_options.subtitles.clone();
        let path = self.media.as_ref().and_then(Media::as_path).map(Path::to_path_buf);
        let (Some(level), Some(path)) = (loading.files, path) else { return };
        let choice = loading.encoding_choice();
        self.queue_files(cx, move || {
            let prepare = |sidecar: Sidecar| {
                Pending::prepare(SubtitleFile::found(sidecar), choice, &loading, false)
            };
            sidecar::find(&path, level).into_iter().map(prepare).collect()
        });
    }

    /// Loads the subtitle files at `paths` linked to `media` (S6.4), unless another media opened
    /// since: as the ones found beside it on opening, or showing the first when the user just
    /// linked them. A file the media already lists isn't loaded twice.
    pub fn load_linked_subtitles(
        &mut self,
        media: &Media,
        paths: Vec<PathBuf>,
        linked: Linked,
        cx: &mut Context<Self>,
    ) {
        if self.media.as_ref() != Some(media) || paths.is_empty() {
            return;
        }
        let Some(video) = media.as_path().map(Path::to_path_buf) else { return };
        let mut show_first = linked == Linked::Now;
        // A file the media already lists (found beside it) shows by its own track.
        if show_first && let Some(track) = self.listed_track(&paths[0]) {
            self.select_track(TrackKind::Subtitle, Some(track), cx);
            show_first = false;
        } else if show_first {
            self.track_choices.user_picked(TrackKind::Subtitle);
        }
        let loading = self.media_options.subtitles.clone();
        self.queue_files(cx, move || {
            let choice = loading.encoding_choice();
            let shows = |index: usize| show_first && index == 0;
            let prepare = |(index, path): (usize, PathBuf)| {
                let file = SubtitleFile::found(sidecar::describe(&path, &video));
                Pending::prepare(file, choice, &loading, shows(index))
            };
            paths.into_iter().enumerate().map(prepare).collect()
        });
    }

    /// Takes the subtitle file at `path` off the open media (its link was removed, S6.4.7).
    /// libVLC can't take a track away, so the player leaves it out from then on, and turns it
    /// off if it shows.
    pub fn remove_subtitle_file(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.info.waiting_files.retain(|pending| pending.file.path != path);
        let (removed, kept) = std::mem::take(&mut self.info.subtitle_files)
            .into_iter()
            .partition(|file| file.path == path);
        self.info.subtitle_files = kept;
        for track in removed.into_iter().filter_map(|file: SubtitleFile| file.track) {
            self.retire(track);
        }
        cx.emit(PlayerEvent::TracksChanged);
        cx.notify();
    }

    /// The track of the listed subtitle file at `path`, once the backend lists it.
    fn listed_track(&self, path: &Path) -> Option<TrackId> {
        self.info.subtitle_files.iter().find(|file| file.path == path).and_then(|file| file.track)
    }

    /// Leaves `track` out from now on, turning it off if it shows.
    fn retire(&mut self, track: TrackId) {
        self.info.retired_tracks.push(track);
        if self.info.tracks.subtitle == Some(track) {
            self.apply_subtitle(None);
        }
        self.info.tracks.all.retain(|listed| listed.id != track);
    }

    /// Adds the subtitle file at `path` to the open media and shows it (a dropped `.srt`).
    /// Returns `false`, and does nothing, when no media is open to add it to.
    pub fn add_subtitle(&mut self, path: &Path, cx: &mut Context<Self>) -> bool {
        if !self.state.is_active() {
            return false;
        }
        // The added file shows: auto-selection must not turn it off.
        self.track_choices.user_picked(TrackKind::Subtitle);
        let (path, loading) = (path.to_path_buf(), self.media_options.subtitles.clone());
        self.queue_files(cx, move || {
            let choice = loading.encoding_choice();
            vec![Pending::prepare(SubtitleFile::new(path), choice, &loading, true)]
        });
        true
    }

    /// Reads the subtitle file whose track is `track` again in `encoding`, or as it would be by
    /// default with `None`, and shows it. libVLC can't take the old track away, so the player
    /// leaves it out from then on.
    pub fn reread_subtitle(
        &mut self,
        track: TrackId,
        encoding: Option<&'static Encoding>,
        cx: &mut Context<Self>,
    ) {
        let files = &mut self.info.subtitle_files;
        let Some(index) = files.iter().position(|file| file.track == Some(track)) else { return };
        let file = files.remove(index);
        self.info.retired_tracks.push(track);
        if self.info.drawn == Some(track) {
            self.info.drawn = None;
        }
        self.track_choices.user_picked(TrackKind::Subtitle);
        let loading = self.media_options.subtitles.clone();
        let choice = encoding.map_or(loading.encoding_choice(), EncodingChoice::Use);
        self.queue_files(cx, move || {
            let file = SubtitleFile { track: None, encoding_chosen: encoding.is_some(), ..file };
            vec![Pending::prepare(file, choice, &loading, true)]
        });
        cx.emit(PlayerEvent::TracksChanged);
        cx.notify();
    }

    /// `tracks` without the tracks of files read again; a retired track that shows counts as
    /// none, until the file's new track does.
    pub(super) fn without_retired(&self, mut tracks: Tracks) -> Tracks {
        let retired = &self.info.retired_tracks;
        tracks.all.retain(|track| !retired.contains(&track.id));
        if tracks.subtitle.is_some_and(|id| retired.contains(&id)) {
            tracks.subtitle = None;
        }
        tracks
    }

    /// Prepares files with `prepare` off the UI thread, then lines them up to load after the
    /// ones asked for before. A newer open drops what is still queued.
    fn queue_files(
        &mut self,
        cx: &mut Context<Self>,
        prepare: impl FnOnce() -> Vec<Pending> + Send + 'static,
    ) {
        let (generation, earlier) = (self.generation, self.subtitle_loads.take());
        let prepared = cx.background_spawn(async move { prepare() });
        self.subtitle_loads = Some(cx.spawn(async move |this, cx| {
            if let Some(earlier) = earlier {
                earlier.await;
            }
            let pending = prepared.await;
            this.update(cx, |player, cx| player.files_ready(generation, pending, cx)).ok();
        }));
    }

    /// Lines up `pending`, prepared for open `generation`, if that media is still open.
    fn files_ready(
        &mut self,
        generation: Generation,
        pending: Vec<Pending>,
        cx: &mut Context<Self>,
    ) {
        if self.generation == generation && self.state.is_active() {
            let listed = |path: &Path| {
                let files = self.info.subtitle_files.iter().map(|file| &file.path);
                let waiting = self.info.waiting_files.iter().map(|pending| &pending.file.path);
                files.chain(waiting).any(|listed| listed == path)
            };
            // A linked file found beside the video too loads once; one asked to show always does.
            let pending: Vec<Pending> =
                pending.into_iter().filter(|p| p.show || !listed(&p.file.path)).collect();
            self.info.waiting_files.extend(pending);
            self.load_waiting_files(cx);
        }
    }

    /// Loads the waiting files, once the backend has listed the media's own tracks.
    pub(super) fn load_waiting_files(&mut self, cx: &mut Context<Self>) {
        if self.info.tracks.all.is_empty() || self.info.waiting_files.is_empty() {
            return;
        }
        for pending in std::mem::take(&mut self.info.waiting_files) {
            let load = pending.load.as_path();
            if pending.show {
                self.with_backend(|backend| backend.add_subtitle(load));
            } else {
                self.with_backend(|backend| backend.load_subtitle(load));
            }
            self.info.subtitle_files.push(pending.file);
        }
        cx.emit(PlayerEvent::TracksChanged);
        cx.notify();
    }
}
