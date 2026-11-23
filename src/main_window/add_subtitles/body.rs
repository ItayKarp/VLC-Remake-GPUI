//! Filling the Add Subtitles dialog's list for its target and origin, off the UI thread: the
//! source folder's subtitle files, the target's videos (a folder's or a playlist's), what is
//! linked to each, and pairing them (S6.4 edge case: a folder of 500 videos).

use std::path::{Path, PathBuf};

use gpui::{App, AppContext as _, Context, PathPromptOptions, Task, Window};

use super::files::Files;
use super::model::{Choice, Origin, Target};
use super::pairs::Pairs;
use super::search::Search;
use super::{AddSubtitlesDialog, Body};
use crate::app::media::Media;
use crate::engine::language;
use crate::engine::sidecar;
use crate::playlists;
use crate::settings;
use crate::subtitle_links::{self, pairing};

/// The source folder From Computer starts in: the file's folder, the folder itself, or the
/// folder of a playlist's first local file (`None` until its items are read).
pub(super) fn default_source(target: &Target) -> Option<PathBuf> {
    match target {
        Target::File(path) => path.parent().map(Path::to_path_buf),
        Target::Folder(folder) => Some(folder.clone()),
        Target::Playlist { .. } => None,
    }
}

impl AddSubtitlesDialog {
    /// Lists what the target and origin offer, read in the background.
    pub(super) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.errors.clear();
        self.status = None;
        self.menu = None;
        self.body = match self.origin {
            Origin::Computer => {
                self.load_computer(window, cx);
                Body::Loading
            }
            Origin::OpenSubtitles => Body::Search(Search::default()),
            Origin::Link => Body::Link,
        };
        if self.origin == Origin::OpenSubtitles {
            self.search(window, cx);
        }
        self.check_conflicts(cx);
        cx.notify();
    }

    /// Reads the source folder, the target's videos and their links, and pairs them.
    fn load_computer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.target().clone();
        let source = self.source.clone();
        let preferred =
            language::parse_list(&settings::get(cx).prefs.subtitles.preferred_languages);
        let videos = videos_of(&target, cx);
        self.loading = Some(cx.spawn_in(window, async move |this, cx| {
            let videos = videos.await;
            let source = source.or_else(|| videos.first().and_then(|v| v.parent()).map(Into::into));
            let linked =
                this.update(cx, |_, cx| subtitle_links::linked_to_each(videos.clone(), cx));
            let Ok(linked) = linked else { return };
            let linked = linked.await;
            let folder = source.clone();
            let candidates = cx
                .background_spawn(async move {
                    folder.map_or_else(Vec::new, |f| sidecar::in_folder(&f))
                })
                .await;
            let body = cx
                .background_spawn(
                    async move { build(&target, &videos, linked, candidates, &preferred) },
                )
                .await;
            this.update(cx, |this, cx| {
                this.source = source;
                this.body = body;
                this.loading = None;
                this.check_conflicts(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    /// The source field's click: asks for another folder and lists its subtitles.
    pub(super) fn pick_source(window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = paths.await else { return };
            let Some(folder) = paths.pop() else { return };
            this.update_in(cx, |this, window, cx| {
                this.source = Some(folder);
                this.reload(window, cx);
            })
            .ok();
        })
        .detach();
    }
}

/// The videos `target` holds, in order: a playlist's local files, a folder's media files.
fn videos_of(target: &Target, cx: &mut App) -> Task<Vec<PathBuf>> {
    match target.clone() {
        Target::File(path) => Task::ready(vec![path]),
        Target::Folder(folder) => {
            cx.background_spawn(async move { subtitle_links::media_in(&folder) })
        }
        Target::Playlist { id, .. } => {
            let items = playlists::items(id, cx);
            cx.background_spawn(async move {
                let items = items.await.unwrap_or_default();
                items.iter().filter_map(Media::as_path).map(Path::to_path_buf).collect()
            })
        }
    }
}

/// The list for `target`: one file's candidates, or every video's pairing.
pub(super) fn build(
    target: &Target,
    videos: &[PathBuf],
    linked: Vec<Vec<PathBuf>>,
    candidates: Vec<PathBuf>,
    preferred: &[language::Language],
) -> Body {
    if let Target::File(video) = target {
        let linked = linked.into_iter().next().unwrap_or_default();
        let mut listed = linked.clone();
        let ranked = pairing::rank_for(video, &candidates);
        listed.extend(ranked.into_iter().filter(|path| !linked.contains(path)));
        let choice = Choice::new(video.clone(), linked, Vec::new());
        return Body::Files(Files { choice, candidates: listed });
    }
    let paired = pairing::pair(videos, &candidates, preferred);
    let paired_any = paired.iter().any(|pairing| !pairing.subtitles.is_empty());
    let choices = paired
        .into_iter()
        .zip(linked)
        .map(|(pairing, linked)| Choice::new(pairing.video, linked, pairing.subtitles))
        .collect();
    Body::Pairs(Pairs { choices, candidates, paired_any })
}
