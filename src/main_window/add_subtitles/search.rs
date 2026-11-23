//! To This File, From OpenSubtitles (S6.4.4, `add_subtitles_to_playlist_folder_03.png`): a
//! search starting from the video's title ("The Wand S01E01"), a language, and the results to
//! check, downloaded on Save. The search itself comes with S6.5
//! ([`crate::subtitle_links::opensubtitles`]); until then the list says it isn't available.

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
use crate::subtitle_links::opensubtitles::{self, Found, Unavailable};
use crate::subtitle_links::query;
use crate::theme::Palette;
use crate::ui::dropdown::Dropdown;
use crate::ui::icon::icon;
use crate::ui::text_input::{Look, TextInput};
use crate::ui::{focus, metrics};

/// The language dropdown's first option.
const ANY_LANGUAGE: &str = "All Languages";
/// The language picked when no preferred language is set.
const DEFAULT_LANGUAGE: &str = "en";

/// What the search found.
#[derive(Clone, Debug, Default)]
pub(super) enum Search {
    /// Waiting for an answer.
    #[default]
    Searching,
    /// The results, and the ids of the checked ones.
    Found { results: Vec<Found>, checked: HashSet<String> },
    /// OpenSubtitles couldn't answer; why, in words.
    Failed(&'static str),
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
    /// Searches OpenSubtitles for what the field says, in the picked language.
    pub(super) fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.controls.search_field.read(cx).text().trim().to_owned();
        let picked = self.controls.language_menu.read(cx).selected();
        let language =
            picked.checked_sub(1).and_then(|index| language::known().into_iter().nth(index));
        self.body = Body::Search(Search::Searching);
        let search =
            cx.background_spawn(async move { opensubtitles::search(&text, language.as_ref()) });
        self.loading = Some(cx.spawn_in(window, async move |this, cx| {
            let answer = search.await;
            this.update(cx, |this, cx| this.searched(answer, cx)).ok();
        }));
        cx.notify();
    }

    /// Shows what a search answered, unless the origin changed meanwhile.
    fn searched(&mut self, answer: Result<Vec<Found>, Unavailable>, cx: &mut Context<Self>) {
        if matches!(self.body, Body::Search(_)) {
            self.body = Body::Search(match answer {
                Ok(results) => Search::Found { results, checked: HashSet::new() },
                Err(Unavailable) => Search::Failed(Unavailable::MESSAGE),
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
            Search::Failed(why) => return note(*why, p).into_any_element(),
            Search::Found { results, .. } if results.is_empty() => {
                return note("Nothing found. Try other words or another language.", p)
                    .into_any_element();
            }
            Search::Found { results, checked } => (results, checked),
        };
        let rows = results.iter().enumerate().map(|(index, found)| {
            spaced(focus::ring(result_row(index, found, checked.contains(&found.id), p, cx), p))
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
        .child(icon_and_name(icons::SUBTITLE_FILE, found.name.clone().into(), p))
        .children(checked.then(|| icon(icons::CHECK, 24., p.accent_fg)))
}
