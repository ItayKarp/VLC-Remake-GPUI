//! The Open Disc dialog (S8.9.1), not designed: **Disc Type** (DVD, Blu-ray, Audio CD, Video
//! CD), **Disc** (the discs in the drives, or a disc folder or image picked with Browse…), and
//! what that kind offers: a DVD or Blu-ray starts at its menus or at a title (and a DVD at a
//! chapter); an audio CD lists its tracks, named from its CD-Text, which Play queues one item
//! each. Picking a disc whose kind its files show picks that kind.
//!
//! The drives are looked at, and an audio CD's tracks read, off the UI thread.

mod open;
mod render;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    PathPromptOptions, SharedString, Subscription, Task, Window,
};

use crate::app::media::Media;
use crate::discs::{self, Disc, DiscKind, Request};
use crate::engine::discs::{CdTrack, audio_cd_tracks};
use crate::main_window::device_dialog::Then;
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::text_input::{self, TextInput};

/// The most digits a title or chapter number takes.
const NUMBER_DIGITS: usize = 3;

/// What is known of an audio CD's tracks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Tracks {
    /// Not asked for: the disc isn't played as an audio CD.
    #[default]
    Unread,
    /// The drive is reading them.
    Reading,
    /// They were read; empty when there are none.
    Read(Vec<CdTrack>),
}

/// Runs with the media to play, once the dialog has closed.
type OnPlay = Then<Vec<Media>>;

/// The Open Disc dialog's view.
pub(super) struct OpenDiscDialog {
    /// The discs found in the drives, then any picked with Browse….
    discs: Vec<Disc>,
    /// Whether the drives were looked at yet.
    looked: bool,
    disc_list: Entity<Dropdown>,
    kind: DiscKind,
    /// Whether a DVD or Blu-ray starts at its menus.
    menus: bool,
    title: Entity<TextInput>,
    chapter: Entity<TextInput>,
    tracks: Tracks,
    /// Looking at the drives, or reading an audio CD's tracks.
    work: Option<Task<()>>,
    on_play: OnPlay,
    focus: FocusHandle,
    _subscriptions: [Subscription; 3],
}

impl EventEmitter<DismissEvent> for OpenDiscDialog {}

impl Focusable for OpenDiscDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl OpenDiscDialog {
    /// The dialog, looking at the drives, handing what to play to `on_play`.
    pub(super) fn new(
        on_play: impl Fn(Vec<Media>, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let disc_list =
            cx.new(|cx| Dropdown::new(tr!("Disc"), vec![tr!("Looking…").into()], 0, window, cx));
        let number = |label: &'static str, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| {
                TextInput::new(window, cx)
                    .with_placeholder(tr!("Any"))
                    .with_label(label)
                    .with_max_len(NUMBER_DIGITS)
            })
        };
        let title = number(tr!("Title"), window, cx);
        let chapter = number(tr!("Chapter"), window, cx);
        let subscriptions = [
            cx.subscribe_in(&disc_list, window, |this, _, picked: &dropdown::Changed, _, cx| {
                this.pick_disc(picked.0, cx);
            }),
            cx.subscribe(&title, |_, _, _: &text_input::Event, cx| cx.notify()),
            cx.subscribe(&chapter, |_, _, _: &text_input::Event, cx| cx.notify()),
        ];
        let mut dialog = Self {
            discs: Vec::new(),
            looked: false,
            disc_list,
            kind: DiscKind::Dvd,
            menus: true,
            title,
            chapter,
            tracks: Tracks::Unread,
            work: None,
            on_play: Rc::new(on_play),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        dialog.look_for_discs(cx);
        dialog
    }

    /// Looks at the drives again, off the UI thread.
    fn look_for_discs(&mut self, cx: &mut Context<Self>) {
        let looking = cx.background_executor().spawn(async { discs::discs() });
        self.work = Some(cx.spawn(async move |this, cx| {
            let found = looking.await;
            this.update(cx, |this, cx| this.found(found, cx)).ok();
        }));
    }

    /// Lists the discs `found`, keeping any picked by hand, and picks the first.
    fn found(&mut self, found: Vec<Disc>, cx: &mut Context<Self>) {
        let picked = self.discs.iter().filter(|disc| disc.device.is_none()).cloned();
        self.discs = found.into_iter().chain(picked).collect();
        self.looked = true;
        self.refresh_list(0, cx);
        self.pick_disc(0, cx);
    }

    /// Shows the discs in the list, `selected` picked.
    fn refresh_list(&mut self, selected: usize, cx: &mut Context<Self>) {
        let options: Vec<SharedString> = if self.discs.is_empty() {
            vec![tr!("No disc found").into()]
        } else {
            self.discs.iter().map(|disc| list_name(disc).into()).collect()
        };
        let empty = self.discs.is_empty();
        self.disc_list.update(cx, |list, cx| {
            list.set_options(options, cx);
            list.set_selected(selected, cx);
            list.set_disabled(empty, cx);
        });
    }

    /// The disc picked in the list.
    fn disc(&self, cx: &App) -> Option<&Disc> {
        self.discs.get(self.disc_list.read(cx).selected())
    }

    /// Picks the disc at `index`, and its kind when its files show it.
    fn pick_disc(&mut self, index: usize, cx: &mut Context<Self>) {
        let kind = self.discs.get(index).and_then(|disc| disc.kind);
        self.set_kind(kind.unwrap_or(self.kind), cx);
        self.read_tracks(cx);
    }

    /// Plays the disc as `kind`.
    pub(super) fn set_kind(&mut self, kind: DiscKind, cx: &mut Context<Self>) {
        if kind != self.kind {
            self.kind = kind;
            // Blu-ray menus need Java, which few have; DVD menus always work.
            self.menus = kind == DiscKind::Dvd;
            self.read_tracks(cx);
        }
        cx.notify();
    }

    /// Reads the audio CD's tracks, if the disc plays as one.
    fn read_tracks(&mut self, cx: &mut Context<Self>) {
        let mrl = self.disc(cx).and_then(discs::audio_cd_mrl);
        let (DiscKind::AudioCd, Some(mrl)) = (self.kind, mrl) else {
            self.tracks = Tracks::Unread;
            return;
        };
        self.tracks = Tracks::Reading;
        let reading = cx.background_executor().spawn(async move { audio_cd_tracks(&mrl) });
        self.work = Some(cx.spawn(async move |this, cx| {
            let tracks = reading.await;
            this.update(cx, |this, cx| {
                this.tracks = Tracks::Read(tracks);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Asks for a disc folder or image, and lists and picks it.
    fn browse(window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some(tr!("Choose").into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else { return };
            let Some(path) = paths.pop() else { return };
            this.update(cx, |this, cx| this.add_picked(&path, cx)).ok();
        })
        .detach();
    }

    /// Lists the folder or image at `path` and picks it.
    fn add_picked(&mut self, path: &Path, cx: &mut Context<Self>) {
        self.discs.push(Disc::picked(path));
        let index = self.discs.len() - 1;
        self.refresh_list(index, cx);
        self.pick_disc(index, cx);
    }

    /// A positive number typed in `field`, if any.
    fn number(field: &Entity<TextInput>, cx: &App) -> Option<u32> {
        field.read(cx).text().trim().parse().ok().filter(|&number| number > 0)
    }

    /// What Play would play, if anything.
    fn media(&self, cx: &App) -> Vec<Media> {
        let Some(disc) = self.disc(cx) else { return Vec::new() };
        let title = Self::number(&self.title, cx);
        let chapter =
            (self.kind == DiscKind::Dvd).then(|| Self::number(&self.chapter, cx)).flatten();
        let request = Request { disc, kind: self.kind, menus: self.menus, title, chapter };
        let tracks = match &self.tracks {
            Tracks::Read(tracks) => tracks.as_slice(),
            Tracks::Unread | Tracks::Reading => &[],
        };
        discs::media(&request, tracks)
    }

    /// Closes the dialog and plays the disc.
    fn play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let media = self.media(cx);
        if !media.is_empty() {
            super::device_dialog::close_with(media, self.on_play.clone(), window, cx);
        }
    }
}

/// A disc as the list shows it: its name, and its kind when known.
fn list_name(disc: &Disc) -> String {
    match disc.kind {
        Some(kind) => format!("{} ({})", disc.name, kind.name()),
        None => disc.name.clone(),
    }
}
