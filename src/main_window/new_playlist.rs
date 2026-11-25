//! The New Playlist dialog (S3.5.1, `dialogs/new_playlist/new_playlist.png`): **Playlist
//! Details** (the title and a custom thumbnail) and **Select Files** (a drop zone that also opens
//! the file picker), then Cancel and Save.
//!
//! Save is enabled once the title shows something and no files are still being added
//! ([`files`]). Cancel, the ✕ and Escape ask before discarding files the user added. Save Queue
//! as Playlist opens it with the queue's items already in it.

mod files;
mod render;
#[cfg(test)]
mod screenshots;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyDownEvent, PathPromptOptions, SharedString, Subscription, Task, WeakEntity, Window,
    prelude::*,
};

use self::files::Adding;
use crate::app::media::Media;
use crate::playlists::{self, Draft, thumbnail};
use crate::settings;
use crate::ui::modal::{Dialog, DialogButton, ModalLayer};
use crate::ui::text_input::{self, TextInput};

/// Runs with what to save, once the dialog has closed.
type OnSave = Rc<dyn Fn(Draft, &mut Window, &mut App)>;

/// The custom thumbnail, as picked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Thumbnail {
    /// None: the playlist gets the automatic one.
    #[default]
    Automatic,
    /// A picked image is being checked.
    Checking,
    /// A checked image.
    Picked(PathBuf),
    /// The last pick was refused, and why.
    Refused(&'static str),
}

/// The New Playlist dialog's view.
pub(super) struct NewPlaylistDialog {
    title: Entity<TextInput>,
    thumbnail: Thumbnail,
    /// The files to save in the playlist, in order.
    files: Vec<Media>,
    /// Whether the user added files here (not just the queue's), so closing asks first.
    added_files: bool,
    /// Files still being resolved (folders scanned, playlists expanded).
    adding: Adding,
    /// What the last addition left out, shown under the drop zone.
    note: Option<SharedString>,
    modals: WeakEntity<ModalLayer>,
    on_save: OnSave,
    focus_handle: FocusHandle,
    /// Space kept between the dialog and the window edges.
    window_margin: f32,
    thumbnail_check: Option<Task<()>>,
    _title_events: Subscription,
}

impl EventEmitter<DismissEvent> for NewPlaylistDialog {}

impl Focusable for NewPlaylistDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl NewPlaylistDialog {
    /// A dialog holding `files` to start with, in `modals`, that hands what to save to `on_save`.
    pub(super) fn new(
        files: Vec<Media>,
        modals: WeakEntity<ModalLayer>,
        on_save: impl Fn(Draft, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let title = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_placeholder(tr!("e.g., Favorite Videos"))
                .with_label(tr!("Playlist title"))
                .with_max_len(playlists::title::MAX_CHARS)
        });
        let title_events =
            cx.subscribe_in(&title, window, |this, _, event, window, cx| match event {
                text_input::Event::Submitted => this.save(window, cx),
                text_input::Event::Changed | text_input::Event::Blurred => cx.notify(),
            });
        Self {
            title,
            thumbnail: Thumbnail::Automatic,
            files,
            added_files: false,
            adding: Adding::default(),
            note: None,
            modals,
            on_save: Rc::new(on_save),
            focus_handle: cx.focus_handle(),
            window_margin: render::WINDOW_MARGIN,
            thumbnail_check: None,
            _title_events: title_events,
        }
    }

    /// Lets the dialog fill a window its own size: the screenshot test's, which is the design's.
    #[cfg(test)]
    fn without_window_margin(mut self) -> Self {
        self.window_margin = 0.;
        self
    }

    /// Whether Save would save: a title that shows something, and no files still coming.
    fn can_save(&self, cx: &App) -> bool {
        playlists::title::is_valid(self.title.read(cx).text())
            && self.adding.is_idle()
            && self.thumbnail != Thumbnail::Checking
    }

    /// Closes the dialog and hands over what to save.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_save(cx) {
            return;
        }
        let thumbnail = match &self.thumbnail {
            Thumbnail::Picked(path) => Some(path.clone()),
            _ => None,
        };
        let draft = Draft {
            title: self.title.read(cx).text().to_owned(),
            thumbnail,
            items: std::mem::take(&mut self.files),
        };
        let on_save = self.on_save.clone();
        cx.emit(DismissEvent);
        window.defer(cx, move |window, cx| on_save(draft, window, cx));
    }

    /// Cancel, ✕ and Escape: closes, after asking when the user added files and confirmations
    /// are on (S5.3.3).
    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.added_files || !settings::get(cx).prefs.startup.show_dialogs {
            cx.emit(DismissEvent);
            return;
        }
        let this = cx.entity().downgrade();
        let discard = DialogButton::new(tr!("Discard")).primary().on_click(move |_, cx| {
            this.update(cx, |_, cx| cx.emit(DismissEvent)).ok();
        });
        let buttons = vec![DialogButton::new(tr!("Keep Editing")), discard];
        let body = |_: &mut Window, _: &mut App| {
            tr!("The files you added to this playlist won't be saved.").into_any_element()
        };
        let confirm = cx.new(|cx| Dialog::new(tr!("Discard this playlist?"), body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(confirm, window, cx)).ok();
    }

    /// Escape asks [`Self::cancel`] instead of the modal layer closing the dialog outright.
    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
            self.cancel(window, cx);
            cx.stop_propagation();
        }
    }

    /// The thumbnail's "+": asks for an image and checks it in the background.
    fn pick_thumbnail(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("Choose").into()),
        });
        let task = cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = paths.await else { return };
            let Some(path) = paths.pop() else { return };
            this.update(cx, |this, cx| {
                this.thumbnail = Thumbnail::Checking;
                cx.notify();
            })
            .ok();
            let checked = path.clone();
            let verdict = cx.background_spawn(async move { thumbnail::check(&checked) }).await;
            this.update(cx, |this, cx| {
                this.thumbnail = match verdict {
                    Ok(()) => Thumbnail::Picked(path),
                    Err(refused) => Thumbnail::Refused(refused.message()),
                };
                cx.notify();
            })
            .ok();
        });
        self.thumbnail_check = Some(task);
    }
}
