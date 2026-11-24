//! The Add Subtitles dialog (S6.4, `dialogs/add_subtitles/*`): a **Target** (To This File, To
//! This Folder, To This Playlist) and an **Origin** (From Computer, From OpenSubtitles, From
//! Link …) over what they list, then Cancel and Save.
//!
//! - To This File, From Computer: the subtitle files in a source folder, several of which can be
//!   checked; the ones already linked come first, with a remove button ([`files`]).
//! - To This Folder or To This Playlist, From Computer: each video paired with subtitles from
//!   the source folder, by episode number and then by name, a picker on each row ([`pairs`]).
//! - From OpenSubtitles: a search and a language over results to check ([`search`]).
//! - From Link: a link to download one subtitle file from ([`link`]).
//!
//! Save links what is checked to its video and unlinks what was unchecked (D16:
//! [`crate::subtitle_links`]), or copies it next to the video first when "Copy next to video" is
//! checked ([`footer`], [`save`]). Lists and pairing are read in the background ([`body`]).

mod body;
mod conflicts;
mod files;
mod footer;
mod header;
mod link;
mod model;
mod pairs;
mod render;
mod save;
#[cfg(test)]
mod screenshots;
mod search;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyDownEvent, Pixels, Point, SharedString, Subscription, Task, Window,
};

pub(super) use self::model::Target;
use self::model::{Copying, Origin, Pair};
use self::search::Search;
use crate::subtitle_links::Outcome;
use crate::ui::dropdown::{self, Dropdown};
use crate::ui::menu::ContextMenu;
use crate::ui::text_input::{self, TextInput};

/// Runs with what Save did, so the main window can apply it to the media playing and say so.
type OnSaved = Rc<dyn Fn(Saved, &mut Window, &mut App)>;

/// What one Save did.
#[derive(Clone, Debug, Default)]
pub(super) struct Saved {
    /// Each subtitle linked, or why it wasn't.
    pub(super) outcomes: Vec<Outcome>,
    /// The subtitles unlinked, by video.
    pub(super) removed: Vec<(PathBuf, PathBuf)>,
}

/// What the dialog lists under its header.
enum Body {
    /// Reading the folder, the links or the playlist.
    Loading,
    /// To This File, From Computer.
    Files(files::Files),
    /// To This Folder or To This Playlist, From Computer.
    Pairs(pairs::Pairs),
    /// From OpenSubtitles.
    Search(Search),
    /// From Link.
    Link,
}

/// A note in the footer: progress, or what went wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Status {
    text: SharedString,
    failed: bool,
}

/// The header row's controls.
struct Controls {
    target_menu: Entity<Dropdown>,
    origin_menu: Entity<Dropdown>,
    search_field: Entity<TextInput>,
    language_menu: Entity<Dropdown>,
    link_field: Entity<TextInput>,
}

impl Controls {
    /// The controls for a dialog offering `targets`, the first picked.
    fn new(targets: &[Target], window: &mut Window, cx: &mut Context<AddSubtitlesDialog>) -> Self {
        let labels = targets.iter().map(|target| target.label().into()).collect();
        let target_menu = cx.new(|cx| {
            Dropdown::new(tr!("Target"), labels, 0, window, cx).with_height(header::CONTROL_HEIGHT)
        });
        let origins = Origin::offered(&targets[0]);
        let labels = origins.iter().map(|origin| origin.label().into()).collect();
        let origin_menu = cx.new(|cx| {
            Dropdown::new(tr!("Origin"), labels, 0, window, cx).with_height(header::CONTROL_HEIGHT)
        });
        let (language_menu, search_field) = search::controls(&targets[0], window, cx);
        let link_field = link::field(window, cx);
        Self { target_menu, origin_menu, search_field, language_menu, link_field }
    }

    /// Follows the dropdowns and fields: a pick lists anew, Enter searches or saves.
    fn subscribe(
        &self,
        window: &mut Window,
        cx: &mut Context<AddSubtitlesDialog>,
    ) -> Vec<Subscription> {
        vec![
            cx.subscribe_in(
                &self.target_menu,
                window,
                |this, _, event: &dropdown::Changed, w, cx| {
                    this.pick_target(event.0, w, cx);
                },
            ),
            cx.subscribe_in(
                &self.origin_menu,
                window,
                |this, _, event: &dropdown::Changed, w, cx| {
                    this.pick_origin(event.0, w, cx);
                },
            ),
            cx.subscribe_in(&self.search_field, window, |this, _, event, window, cx| {
                if matches!(event, text_input::Event::Submitted) {
                    this.search(window, cx);
                }
            }),
            cx.subscribe_in(&self.link_field, window, |this, _, event, window, cx| match event {
                text_input::Event::Submitted => this.save(window, cx),
                _ => this.link_changed(cx),
            }),
        ]
    }
}

/// The Add Subtitles dialog's view.
pub(super) struct AddSubtitlesDialog {
    targets: Vec<Target>,
    target: usize,
    origin: Origin,
    controls: Controls,
    /// From Computer's source folder.
    source: Option<PathBuf>,
    body: Body,
    copying: Copying,
    /// Rows that couldn't be saved, and why.
    errors: HashMap<Pair, String>,
    status: Option<Status>,
    /// A row's picker or conflict menu, where it opened.
    menu: Option<(Point<Pixels>, Entity<ContextMenu>, Subscription)>,
    on_saved: OnSaved,
    focus_handle: FocusHandle,
    /// Space kept between the dialog and the window edges.
    window_margin: f32,
    loading: Option<Task<()>>,
    conflict_check: Option<Task<()>>,
    saving: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for AddSubtitlesDialog {}

impl Focusable for AddSubtitlesDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl AddSubtitlesDialog {
    /// A dialog adding subtitles to `target` (and the targets it offers), handing what Save did
    /// to `on_saved`.
    pub(super) fn new(
        target: Target,
        on_saved: impl Fn(Saved, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let targets = target.offered();
        let controls = Controls::new(&targets, window, cx);
        let subscriptions = controls.subscribe(window, cx);
        let mut dialog = Self {
            targets,
            target: 0,
            origin: Origin::Computer,
            controls,
            source: None,
            body: Body::Loading,
            copying: Copying::default(),
            errors: HashMap::new(),
            status: None,
            menu: None,
            on_saved: Rc::new(on_saved),
            focus_handle: cx.focus_handle(),
            window_margin: render::WINDOW_MARGIN,
            loading: None,
            conflict_check: None,
            saving: None,
            _subscriptions: subscriptions,
        };
        dialog.source = body::default_source(dialog.target());
        dialog.reload(window, cx);
        dialog
    }

    /// The target picked in the Target dropdown.
    fn target(&self) -> &Target {
        &self.targets[self.target]
    }

    /// A Target picked: offers its origins, and lists it.
    fn pick_target(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.target = index.min(self.targets.len() - 1);
        let origins = Origin::offered(self.target());
        if !origins.contains(&self.origin) {
            self.origin = Origin::Computer;
        }
        let labels = origins.iter().map(|origin| origin.label().into()).collect();
        let selected = origins.iter().position(|origin| *origin == self.origin).unwrap_or(0);
        self.controls.origin_menu.update(cx, |menu, cx| {
            menu.set_options(labels, cx);
            menu.set_selected(selected, cx);
        });
        self.source = body::default_source(self.target());
        self.reload(window, cx);
    }

    /// An Origin picked: lists what it offers.
    fn pick_origin(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let origins = Origin::offered(self.target());
        self.origin = origins.get(index).copied().unwrap_or(Origin::Computer);
        self.reload(window, cx);
    }

    /// Whether Save would save something now.
    fn can_save(&self, cx: &App) -> bool {
        if self.saving.is_some() {
            return false;
        }
        match &self.body {
            Body::Loading => false,
            Body::Files(files) => files.choice.change(|_| None).is_some(),
            Body::Pairs(pairs) => pairs.choices.iter().any(|c| c.change(|_| None).is_some()),
            Body::Search(search) => search.has_checked(),
            Body::Link => !self.controls.link_field.read(cx).text().trim().is_empty(),
        }
    }

    /// Cancel, ✕ and Escape: closes, stopping a download or search in flight.
    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.saving = None;
        self.loading = None;
        cx.emit(DismissEvent);
    }

    /// Escape closes the row menu first, then the dialog.
    fn on_key_down(&mut self, event: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
            if self.menu.take().is_some() {
                cx.notify();
            } else {
                self.cancel(cx);
            }
            cx.stop_propagation();
        }
    }

    /// Shows `text` in the footer, in the danger color when something `failed`.
    fn report(&mut self, text: impl Into<SharedString>, failed: bool, cx: &mut Context<Self>) {
        self.status = Some(Status { text: text.into(), failed });
        cx.notify();
    }

    /// Lets the dialog fill a window its own size: the screenshot test's, which is the design's.
    #[cfg(test)]
    fn without_window_margin(mut self) -> Self {
        self.window_margin = 0.;
        self
    }
}
