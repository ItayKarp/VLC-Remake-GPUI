//! The main window's side of linked subtitles (S6.4): opening the Add Subtitles dialog from the
//! library's menus and the track popover's pencil, giving the media playing what Save linked or
//! unlinked at once, and loading a media's linked subtitles whenever it opens.

use std::path::{Path, PathBuf};

use gpui::{App, AppContext as _, Context, Window};

use super::MainWindow;
use super::add_subtitles::{AddSubtitlesDialog, Saved, Target};
use crate::app::media::Media;
use crate::engine::player::{Linked, Player};
use crate::subtitle_links;
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// Opens the Add Subtitles dialog on `target`.
    pub(super) fn show_add_subtitles(
        &mut self,
        target: Target,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let main = cx.entity().downgrade();
        let on_saved = move |saved: Saved, _: &mut Window, cx: &mut App| {
            main.update(cx, |main, cx| main.apply_saved_subtitles(&saved, cx)).ok();
        };
        let dialog = cx.new(|cx| AddSubtitlesDialog::new(target, on_saved, window, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// The track popover's pencil: the dialog on the local file playing.
    pub(super) fn edit_playing_subtitles(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let Some(path) =
            player.read(cx).media().and_then(|media| media.as_path()).map(Path::to_path_buf)
        else {
            return;
        };
        self.show_add_subtitles(Target::File(path), window, cx);
    }

    /// Gives the media playing the subtitles `saved` linked to it (the first shows) and takes
    /// away the ones it unlinked, then says how many were added.
    fn apply_saved_subtitles(&mut self, saved: &Saved, cx: &mut Context<Self>) {
        let linked: Vec<(PathBuf, PathBuf)> = saved
            .outcomes
            .iter()
            .filter_map(|outcome| Some((outcome.video.clone(), outcome.linked.clone().ok()?)))
            .collect();
        if let Some(player) = self.loaded_player(cx)
            && let Some(media) = player.read(cx).media().cloned()
            && let Some(playing) = media.as_path().map(Path::to_path_buf)
        {
            let added = of_video(&linked, &playing);
            let removed = of_video(&saved.removed, &playing);
            player.update(cx, |player, cx| relink(player, &media, added, &removed, cx));
        }
        let message = match linked.len() {
            0 if saved.removed.is_empty() => return,
            0 => "Removed the subtitles.".to_owned(),
            1 => "Added 1 subtitle.".to_owned(),
            n => format!("Added {n} subtitles."),
        };
        self.show_toast(Toast::new(Level::Success, message), cx);
    }

    /// Loads the subtitles linked to the media the player opens (a player event).
    pub(super) fn load_linked_subtitles(&self, cx: &mut Context<Self>) {
        let Some(player) = self.loaded_player(cx) else { return };
        let Some(media) = player.read(cx).media().cloned() else { return };
        let Some(path) = media.as_path() else { return };
        let linked = subtitle_links::linked(path, cx);
        cx.spawn(async move |_, cx| {
            let paths = linked.await;
            player.update(cx, |player, cx| {
                player.load_linked_subtitles(&media, paths, Linked::OnOpen, cx);
            });
        })
        .detach();
    }
}

/// Takes the `removed` subtitles off `media` playing in `player`, and loads the `added` ones.
fn relink(
    player: &mut Player,
    media: &Media,
    added: Vec<PathBuf>,
    removed: &[PathBuf],
    cx: &mut Context<Player>,
) {
    for subtitle in removed {
        player.remove_subtitle_file(subtitle, cx);
    }
    player.load_linked_subtitles(media, added, Linked::Now, cx);
}

/// The subtitles of `pairs` that belong to `video`.
fn of_video(pairs: &[(PathBuf, PathBuf)], video: &Path) -> Vec<PathBuf> {
    pairs.iter().filter(|(of, _)| of == video).map(|(_, subtitle)| subtitle.clone()).collect()
}
