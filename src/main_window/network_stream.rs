//! The Open Network Stream dialog (S4.11.1, `dialogs/open_network_stream`): **Network URL**, a
//! field of several lines taking a link per line, and **Playlist Name (optional)**, then Cancel
//! and Save.
//!
//! The links are read as they are typed ([`links`]): what isn't a media link, repeats, and
//! links with a password (never stored) are noted under the field. Save is enabled once there
//! is a link to act on. Without a name, Save plays the links and lists them in Network → Recent
//! Streams; with one, it saves them as a playlist (S4.11.2), which Network lists too.

mod open;
mod render;
#[cfg(test)]
mod screenshots;
#[cfg(test)]
mod tests;

use std::rc::Rc;

use gpui::{
    App, AppContext as _, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    SharedString, Subscription, Window,
};

use crate::network::{self, links};
use crate::playlists;
use crate::ui::text_input::{self, TextInput};

/// How many of the links field's lines show at once.
const URL_ROWS: usize = 8;

/// What Save hands over.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Saved {
    /// Play these links now.
    Play(Vec<String>),
    /// Save these links as a playlist with this title.
    Playlist { title: String, urls: Vec<String> },
}

/// Runs with what to do, once the dialog has closed.
type OnSave = Rc<dyn Fn(Saved, &mut Window, &mut App)>;

/// The Open Network Stream dialog's view.
pub(super) struct NetworkStreamDialog {
    urls: Entity<TextInput>,
    name: Entity<TextInput>,
    /// What the links field holds, read again on every change.
    links: links::Links,
    on_save: OnSave,
    /// Space kept between the dialog and the window edges.
    window_margin: f32,
    _subscriptions: [Subscription; 2],
}

impl EventEmitter<DismissEvent> for NetworkStreamDialog {}

impl Focusable for NetworkStreamDialog {
    /// The links field, so typing starts there.
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.urls.focus_handle(cx)
    }
}

impl NetworkStreamDialog {
    /// An empty dialog that hands what to do to `on_save`.
    pub(super) fn new(
        on_save: impl Fn(Saved, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let urls = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_rows(URL_ROWS)
                .with_placeholder(tr!("e.g., https://www.example.com/stream.avi"))
                .with_label(tr!("Network URL"))
        });
        let name = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_placeholder(tr!("e.g., Favorite TV Channels"))
                .with_label(tr!("Playlist name"))
                .with_max_len(playlists::title::MAX_CHARS)
        });
        let subscriptions = [
            cx.subscribe_in(&urls, window, |this, urls, event, window, cx| {
                if *event == text_input::Event::Changed {
                    this.links = links::read(urls.read(cx).text());
                }
                this.on_field_event(event, window, cx);
            }),
            cx.subscribe_in(&name, window, |this, _, event, window, cx| {
                this.on_field_event(event, window, cx);
            }),
        ];
        Self {
            urls,
            name,
            links: links::Links::default(),
            on_save: Rc::new(on_save),
            window_margin: render::WINDOW_MARGIN,
            _subscriptions: subscriptions,
        }
    }

    /// Lets the dialog fill a window its own size: the screenshot test's, which is the design's.
    #[cfg(test)]
    fn without_window_margin(mut self) -> Self {
        self.window_margin = 0.;
        self
    }

    /// Enter in the name field (Cmd/Ctrl+Enter in the links field) saves; any change redraws.
    fn on_field_event(
        &mut self,
        event: &text_input::Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            text_input::Event::Submitted => self.save(window, cx),
            text_input::Event::Changed | text_input::Event::Blurred => cx.notify(),
        }
    }

    /// The playlist title typed, if it shows something.
    fn title(&self, cx: &App) -> Option<String> {
        let typed = self.name.read(cx).text();
        playlists::title::is_valid(typed).then(|| typed.to_owned())
    }

    /// What Save would do, if it would do anything: a playlist saves only the links it may
    /// store.
    fn saved(&self, cx: &App) -> Option<Saved> {
        let saved = match self.title(cx) {
            None => Saved::Play(self.links.urls.clone()),
            Some(title) => Saved::Playlist { title, urls: network::storable(&self.links.urls) },
        };
        let urls = match &saved {
            Saved::Play(urls) | Saved::Playlist { urls, .. } => urls,
        };
        (!urls.is_empty()).then_some(saved)
    }

    /// Closes the dialog and hands over what to do.
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(saved) = self.saved(cx) else { return };
        let on_save = self.on_save.clone();
        cx.emit(DismissEvent);
        window.defer(cx, move |window, cx| on_save(saved, window, cx));
    }

    /// The notes under the links field: what is left out, and why.
    fn notes(&self, cx: &App) -> Vec<SharedString> {
        let mut notes = Vec::new();
        notes.extend(refused_note(&self.links.refused));
        notes.extend(match self.links.duplicates {
            0 => None,
            n => Some(
                trn!(
                    "{count} repeated link is left out.",
                    "{count} repeated links are left out.",
                    n
                )
                .into(),
            ),
        });
        let with_password = self.links.urls.len() - network::storable(&self.links.urls).len();
        if with_password > 0 {
            notes.push(if self.title(cx).is_some() {
                tr!("Links with a password can't be saved in a playlist, so they are left out.")
                    .into()
            } else {
                tr!("Links with a password play, but aren't kept in Recent Streams.").into()
            });
        }
        notes
    }
}

/// What the notes say about `refused` words: the first one, masked, and how many there are.
fn refused_note(refused: &[String]) -> Option<SharedString> {
    let first = links::masked(refused.first()?);
    Some(match refused.len() {
        1 => tr!("“{first}” isn't a media link, so it is left out.", first).into(),
        n => trn!(
            "{count} entry, like “{first}”, isn't a media link, so it is left out.",
            "{count} entries, like “{first}”, aren't media links, so they are left out.",
            n,
            first
        )
        .into(),
    })
}
