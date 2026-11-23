//! Save in the Add Subtitles dialog: downloads first for From OpenSubtitles and From Link, then
//! links what is checked and unlinks what was unchecked ([`crate::subtitle_links::save`]),
//! copying next to the video when asked.
//!
//! When every row saved, the dialog closes; otherwise it stays open with each failed row saying
//! why (the video or subtitle is gone, the disk is full, …) and the ones that saved counted as
//! linked, so Save again only retries the rest. Cancel stops what is in flight.

use std::path::PathBuf;

use gpui::{AppContext as _, Context, DismissEvent, Window};

use super::model::Target;
use super::{AddSubtitlesDialog, Body, Saved};
use crate::settings::{self, Locations};
use crate::subtitle_links::opensubtitles;
use crate::subtitle_links::{self, Addition, Change, Outcome, download};

/// What a From OpenSubtitles or From Link Save downloads first.
enum Downloads {
    /// The checked results, and the OpenSubtitles username to download as.
    OpenSubtitles(Vec<opensubtitles::Found>, String),
    Link(url::Url),
}

impl AddSubtitlesDialog {
    /// Save: downloads what needs it, then saves the links.
    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save(cx) {
            return;
        }
        let downloads = match &self.body {
            Body::Files(_) | Body::Pairs(_) => {
                let changes = self.changes();
                self.run_save(changes, window, cx);
                return;
            }
            Body::Search(search) => {
                let username = settings::get(cx).prefs.privacy.opensubtitles.username.clone();
                Downloads::OpenSubtitles(search.checked(), username)
            }
            Body::Link => match download::parse_link(self.controls.link_field.read(cx).text()) {
                Ok(url) => Downloads::Link(url),
                Err(failure) => return self.report(failure.message(), true, cx),
            },
            Body::Loading => return,
        };
        self.download_then_save(downloads, window, cx);
    }

    /// What Save changes for each video of the list.
    fn changes(&self) -> Vec<Change> {
        let choices: &[_] = match &self.body {
            Body::Files(files) => std::slice::from_ref(&files.choice),
            Body::Pairs(pairs) => &pairs.choices,
            _ => &[],
        };
        choices
            .iter()
            .filter_map(|choice| choice.change(|pair| self.copying.copy_of(pair)))
            .collect()
    }

    /// Downloads `downloads` in the background, then links what came down to the file.
    fn download_then_save(
        &mut self,
        downloads: Downloads,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Target::File(video) = self.target().clone() else { return };
        let proxy = settings::proxy_url(&settings::get(cx).prefs.network.http_proxy);
        self.report("Downloading…", false, cx);
        let fetch = cx.background_spawn(async move { fetch(downloads, proxy.as_ref()) });
        self.saving = Some(cx.spawn_in(window, async move |this, cx| {
            let fetched = fetch.await;
            this.update_in(cx, |this, window, cx| this.downloaded(video, fetched, window, cx)).ok();
        }));
        cx.notify();
    }

    /// Links to `video` what was `fetched`, and says what wasn't.
    fn downloaded(
        &mut self,
        video: PathBuf,
        fetched: Vec<Result<PathBuf, String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.saving = None;
        let copy = self.copying.on.then(Default::default);
        let mut add = Vec::new();
        for result in fetched {
            match result {
                Ok(subtitle) => add.push(Addition { subtitle, copy }),
                Err(why) => self.report(why, true, cx),
            }
        }
        if !add.is_empty() {
            self.run_save(vec![Change { video, add, remove: Vec::new() }], window, cx);
        }
    }

    /// Saves `changes`, then reports what happened.
    fn run_save(&mut self, changes: Vec<Change>, window: &mut Window, cx: &mut Context<Self>) {
        if changes.is_empty() {
            return;
        }
        let removed: Vec<(PathBuf, PathBuf)> = changes
            .iter()
            .flat_map(|change| {
                change.remove.iter().map(|subtitle| (change.video.clone(), subtitle.clone()))
            })
            .collect();
        self.report("Saving…", false, cx);
        let saving = subtitle_links::save(changes, cx);
        self.saving = Some(cx.spawn_in(window, async move |this, cx| {
            let outcomes = saving.await;
            this.update_in(cx, |this, window, cx| {
                this.finish_saving(Saved { outcomes, removed }, window, cx);
            })
            .ok();
        }));
        cx.notify();
    }

    /// Hands what was saved to the main window, then closes, or shows the rows that failed.
    fn finish_saving(&mut self, saved: Saved, window: &mut Window, cx: &mut Context<Self>) {
        self.saving = None;
        self.record(&saved.outcomes);
        let failed = saved.outcomes.iter().filter(|outcome| outcome.linked.is_err()).count();
        let on_saved = self.on_saved.clone();
        window.defer(cx, move |window, cx| on_saved(saved, window, cx));
        match failed {
            0 => cx.emit(DismissEvent),
            1 => self.report("1 subtitle couldn't be added; its row says why.", true, cx),
            n => self.report(
                format!("{n} subtitles couldn't be added; their rows say why."),
                true,
                cx,
            ),
        }
    }

    /// Counts the saved rows as linked and notes why the others failed.
    fn record(&mut self, outcomes: &[Outcome]) {
        self.errors.clear();
        for outcome in outcomes {
            let pair = (outcome.video.clone(), outcome.subtitle.clone());
            match &outcome.linked {
                Ok(linked) => self
                    .choice_of(&outcome.video)
                    .into_iter()
                    .for_each(|c| c.saved(&outcome.subtitle, linked)),
                Err(why) => {
                    self.errors.insert(pair, why.clone());
                }
            }
        }
        let choices: &mut [_] = match &mut self.body {
            Body::Files(files) => std::slice::from_mut(&mut files.choice),
            Body::Pairs(pairs) => &mut pairs.choices,
            _ => &mut [],
        };
        choices.iter_mut().for_each(super::model::Choice::removals_saved);
        // A copy now stands for the file it was made from: list it, checked.
        if let Body::Files(files) = &mut self.body {
            let missing: Vec<PathBuf> = files
                .choice
                .chosen
                .iter()
                .filter(|chosen| !files.candidates.contains(chosen))
                .cloned()
                .collect();
            files.candidates.extend(missing);
        }
    }

    /// The list's choice for `video`.
    fn choice_of(&mut self, video: &std::path::Path) -> Option<&mut super::model::Choice> {
        match &mut self.body {
            Body::Files(files) => Some(&mut files.choice).filter(|choice| choice.video == video),
            Body::Pairs(pairs) => pairs.choices.iter_mut().find(|choice| choice.video == video),
            _ => None,
        }
    }
}

/// Downloads `downloads` into the app's `subtitles/` folder, through `proxy` when one is set,
/// answering each file's path or why it didn't come. Blocks on the network.
fn fetch(downloads: Downloads, proxy: Option<&url::Url>) -> Vec<Result<PathBuf, String>> {
    let Some(folder) = Locations::resolve().map(|locations| locations.subtitles()) else {
        return vec![Err("There's no folder to save it in.".to_owned())];
    };
    match downloads {
        Downloads::Link(url) => {
            vec![download::download(&url, &folder, proxy).map_err(|failure| failure.message())]
        }
        Downloads::OpenSubtitles(found, username) => {
            let into = opensubtitles::Downloads { folder, username, proxy: proxy.cloned() };
            found
                .iter()
                .map(|found| {
                    opensubtitles::download(found, &into)
                        .map_err(|failure| format!("{}: {}", found.title(), failure.message()))
                })
                .collect()
        }
    }
}
