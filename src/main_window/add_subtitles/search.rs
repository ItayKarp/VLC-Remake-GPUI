//! To This File, From OpenSubtitles (S6.4.4, `add_subtitles_to_playlist_folder_03.png`): a
//! search starting from the video's title ("The Wand S01E01"), a language, and the results to
//! check, downloaded on Save ([`crate::subtitle_links::opensubtitles`], S6.5). Each result
//! names its release, language, rating and downloads, the ones made for this file first.
//!
//! Searching sends the file's hash and name to OpenSubtitles, so until the user allows it
//! (S6.5.4, also Settings → Privacy → Search OpenSubtitles) the list asks first.

use std::collections::HashSet;

use gpui::{
    AnyElement, AppContext as _, ClickEvent, Context, Div, Entity, SharedString, Stateful, Window,
    accesskit, div, prelude::*, px,
};

use super::files::spaced;
use super::header::CONTROL_HEIGHT;
use super::model::Target;
use super::render::{PADDING, ROW_HEIGHT, icon_and_name, note};
use super::{AddSubtitlesDialog, Body};
use crate::assets::icons;
use crate::engine::language::{self, Language};
use crate::settings;
use crate::subtitle_links::opensubtitles::{self, Failure, Found, Query};
use crate::subtitle_links::query;
use crate::theme::Palette;
use crate::theme::text_px;
use crate::ui::button::{Button, ButtonStyle};
use crate::ui::dropdown::Dropdown;
use crate::ui::icon::icon;
use crate::ui::text_input::{Look, TextInput};
use crate::ui::{focus, metrics};

/// The language dropdown's first option.
const ANY_LANGUAGE: &str = "All Languages";
/// The language picked when no preferred language is set.
const DEFAULT_LANGUAGE: &str = "en";
/// What the consent note says (S6.5.4).
const CONSENT: &str = "Searching OpenSubtitles.com sends it this video's file hash and the \
                       words you search for. You can turn this off in Settings → Privacy.";
/// The consent button's width.
const CONSENT_BUTTON_WIDTH: f32 = 160.;

/// What the search found.
#[derive(Clone, Debug, Default)]
pub(super) enum Search {
    /// Waiting for an answer.
    #[default]
    Searching,
    /// Waiting for the user to allow sending the search to OpenSubtitles.
    Consent,
    /// The results, and the ids of the checked ones.
    Found { results: Vec<Found>, checked: HashSet<String> },
    /// OpenSubtitles couldn't answer; why, in words.
    Failed(String),
}

impl Search {
    /// Whether a result is checked.
    pub(super) fn has_checked(&self) -> bool {
        matches!(self, Self::Found { checked, .. } if !checked.is_empty())
    }

    /// The checked results, in the list's order.
    pub(super) fn checked(&self) -> Vec<Found> {
        match self {
            Self::Found { results, checked } => {
                results.iter().filter(|found| checked.contains(&found.id)).cloned().collect()
            }
            _ => Vec::new(),
        }
    }
}

/// The language dropdown and the search field for `target`: its title, and the first preferred
/// language (English without one).
pub(super) fn controls(
    target: &Target,
    window: &mut Window,
    cx: &mut Context<AddSubtitlesDialog>,
) -> (Entity<Dropdown>, Entity<TextInput>) {
    let languages = language::known();
    let preferred = language::parse_list(&settings::get(cx).prefs.subtitles.preferred_languages);
    let wanted = preferred.first().cloned().or_else(|| Language::recognize(DEFAULT_LANGUAGE));
    let selected =
        wanted.and_then(|wanted| languages.iter().position(|l| *l == wanted)).map_or(0, |i| i + 1);
    let options = std::iter::once(ANY_LANGUAGE.into())
        .chain(languages.iter().map(|language| SharedString::from(language.name())))
        .collect();
    let menu = cx.new(|cx| {
        Dropdown::new("Language", options, selected, window, cx).with_height(CONTROL_HEIGHT)
    });
    let title = match target {
        Target::File(path) => query::for_video(path),
        _ => String::new(),
    };
    let field = cx.new(|cx| {
        TextInput::new(window, cx)
            .with_look(Look::Bare)
            .with_text(title)
            .with_placeholder("Search by title")
            .with_label("Search OpenSubtitles")
    });
    (menu, field)
}

impl AddSubtitlesDialog {
    /// Searches OpenSubtitles for what the field says, in the picked language, once the user
    /// has allowed it.
    pub(super) fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !settings::get(cx).prefs.privacy.opensubtitles.search {
            self.body = Body::Search(Search::Consent);
            cx.notify();
            return;
        }
        let query = self.query(cx);
        self.body = Body::Search(Search::Searching);
        let search = cx.background_spawn(async move { opensubtitles::search(&query) });
        self.loading = Some(cx.spawn_in(window, async move |this, cx| {
            let answer = search.await;
            this.update(cx, |this, cx| this.searched(answer, cx)).ok();
        }));
        cx.notify();
    }

    /// What the header asks for, with the video to hash.
    fn query(&self, cx: &Context<Self>) -> Query {
        let picked = self.controls.language_menu.read(cx).selected();
        let video = match self.target() {
            Target::File(path) => Some(path.clone()),
            _ => None,
        };
        Query {
            text: self.controls.search_field.read(cx).text().trim().to_owned(),
            language: picked
                .checked_sub(1)
                .and_then(|index| language::known().into_iter().nth(index)),
            video,
            proxy: settings::proxy_url(&settings::get(cx).prefs.network.http_proxy),
        }
    }

    /// The consent note's button: allows OpenSubtitles from now on, and searches.
    fn allow_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        settings::update(cx, |settings| settings.prefs.privacy.opensubtitles.search = true);
        self.search(window, cx);
    }

    /// Shows what a search answered, unless the origin changed meanwhile.
    fn searched(&mut self, answer: Result<Vec<Found>, Failure>, cx: &mut Context<Self>) {
        if matches!(self.body, Body::Search(_)) {
            self.body = Body::Search(match answer {
                Ok(results) => Search::Found { results, checked: HashSet::new() },
                Err(failure) => Search::Failed(failure.message()),
            });
        }
        cx.notify();
    }

    /// The results, or what is happening instead.
    pub(super) fn render_search(
        search: &Search,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (results, checked) = match search {
            Search::Searching => return note("Searching OpenSubtitles…", p).into_any_element(),
            Search::Consent => return consent(p, cx).into_any_element(),
            Search::Failed(why) => return note(why.clone(), p).into_any_element(),
            Search::Found { results, .. } if results.is_empty() => {
                return note("Nothing found. Try other words or another language.", p)
                    .into_any_element();
            }
            Search::Found { results, checked } => (results, checked),
        };
        let rows = results.iter().enumerate().map(|(index, found)| {
            spaced(focus::outline_ring(
                result_row(index, found, checked.contains(&found.id), p, cx),
                p,
            ))
        });
        div()
            .id("add-subtitles-results")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(PADDING))
            .children(rows)
            .into_any_element()
    }

    /// Checks or unchecks result `id`.
    fn toggle_result(&mut self, id: &str, cx: &mut Context<Self>) {
        let Body::Search(Search::Found { checked, .. }) = &mut self.body else { return };
        if !checked.remove(id) {
            checked.insert(id.to_owned());
        }
        cx.notify();
    }
}

/// Result `index`: its name, and the check when `checked`; a click checks or unchecks it.
fn result_row(
    index: usize,
    found: &Found,
    checked: bool,
    p: &Palette,
    cx: &mut Context<AddSubtitlesDialog>,
) -> Stateful<Div> {
    let id = found.id.clone();
    div()
        .id(("add-subtitles-result", index))
        .tab_index(0)
        .key_context(focus::CONTROL)
        .role(accesskit::Role::CheckBox)
        .aria_toggled(checked.into())
        .h(px(ROW_HEIGHT))
        .flex()
        .items_center()
        .justify_between()
        .pl(px(18.))
        .pr(px(16.))
        .rounded(px(metrics::RADIUS + 2.))
        .cursor_pointer()
        .when(checked, |row| row.bg(p.sunken))
        .hover(|style| style.bg(p.hover))
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.toggle_result(&id, cx)))
        .gap(px(16.))
        .child(icon_and_name(icons::SUBTITLE_FILE, found.title().to_owned().into(), p))
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(16.))
                .text_size(text_px(metrics::TEXT - 1.))
                .text_color(p.text_muted)
                .child(found.details())
                .child(
                    div()
                        .w(px(24.))
                        .children(checked.then(|| icon(icons::CHECK, 24., p.accent_fg))),
                ),
        )
}

/// The consent note (S6.5.4): what a search sends, and a button allowing it.
fn consent(p: &Palette, cx: &mut Context<AddSubtitlesDialog>) -> Div {
    let allow = Button::new("add-subtitles-allow-opensubtitles", "Allow and Search")
        .style(ButtonStyle::Primary)
        .width(CONSENT_BUTTON_WIDTH)
        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.allow_search(window, cx)));
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(note(CONSENT, p))
        .child(div().px(px(PADDING + 6.)).child(allow))
}
