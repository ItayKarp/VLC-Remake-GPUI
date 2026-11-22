//! What the listing shows when a place lists nothing (S4.7.6). Not designed: a muted icon, a
//! line saying what is missing, a hint at how to fill it, and, for a search of one section, a
//! button that searches everywhere instead.
//!
//! A search says "No results for “…”", a folder "This folder is empty", History "Nothing watched
//! yet", Albums, Artists and Genres that no songs have been read, Playlists that none were
//! made. Anything else says "Nothing here yet."

use gpui::{ClickEvent, Context, Div, SharedString, div, prelude::*, px};

use super::{INSET, MainWindow};
use crate::assets::icons;
use crate::navigation::route::{MusicRoute, Route, Search, Section};
use crate::theme::Palette;
use crate::ui::button::Button;
use crate::ui::icon::icon;

/// The icon's size, the gap under it and between the lines, and the text sizes.
const ICON: f32 = 32.;
const GAP: f32 = 8.;
const TITLE_TEXT: f32 = 15.;
const HINT_TEXT: f32 = 13.;
/// Air above the block.
const TOP: f32 = 24.;

/// What an empty place says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EmptyState {
    icon: &'static str,
    title: String,
    hint: &'static str,
    /// Offers to search everywhere: a search of one section found nothing.
    everywhere: bool,
}

impl EmptyState {
    /// What `route` says when it lists nothing.
    pub(super) fn of(route: &Route) -> Self {
        let state = |icon, title: &str, hint| Self {
            icon,
            title: title.to_owned(),
            hint,
            everywhere: false,
        };
        match route {
            Route::Search(search) => searched(search),
            Route::Videos(Some(_)) => {
                state(icons::FOLDER, "This folder is empty", "It holds no videos or folders.")
            }
            Route::Music(MusicRoute::Songs(Some(_))) => {
                state(icons::FOLDER, "This folder is empty", "It holds no songs or folders.")
            }
            Route::Music(
                MusicRoute::Albums(None) | MusicRoute::Artists(None) | MusicRoute::Genres(None),
            ) => state(icons::MUSIC, "No music yet", NO_SONGS),
            Route::History => {
                state(icons::RECENT, "Nothing watched yet", "What you play shows up here.")
            }
            Route::Playlists(None) => state(
                icons::PLAYLIST,
                "No playlists yet",
                "Save the queue as a playlist to see it here.",
            ),
            _ => state(icons::FOLDER, "Nothing here yet", ""),
        }
    }
}

/// The hint of Music's empty categories.
const NO_SONGS: &str = "Songs in your music folders show up here once the library has read them.";

fn searched(search: &Search) -> EmptyState {
    let everywhere = search.scope == Section::Home;
    EmptyState {
        icon: icons::SEARCH,
        title: format!("No results for “{}”", search.query),
        hint: if everywhere {
            "Check the spelling, or try fewer words."
        } else {
            "Try fewer words, or search the whole library."
        },
        everywhere: !everywhere,
    }
}

impl MainWindow {
    /// The block saying why the current place lists nothing.
    pub(super) fn render_empty(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let state = EmptyState::of(self.library_panel.history.current());
        let title = div()
            .text_size(px(TITLE_TEXT))
            .text_color(p.text)
            .child(SharedString::from(state.title));
        let hint = (!state.hint.is_empty())
            .then(|| div().text_size(px(HINT_TEXT)).text_color(p.text_muted).child(state.hint));
        let everywhere = state.everywhere.then(|| {
            Button::new("library-empty-everywhere", "Search Everywhere").on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.search_everywhere(cx);
                },
            ))
        });
        div()
            .pt(px(TOP))
            .px(px(INSET))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(GAP))
            .text_center()
            .child(icon(state.icon, ICON, p.text_muted))
            .child(title)
            .children(hint)
            .children(everywhere.map(|button| div().mt(px(GAP)).child(button)))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn a_search_of_one_section_offers_everywhere() {
        let search = |scope| Route::Search(Search { query: "wand".into(), scope });
        let videos = EmptyState::of(&search(Section::Videos));
        assert_eq!(videos.title, "No results for “wand”");
        assert!(videos.everywhere);
        assert!(!EmptyState::of(&search(Section::Home)).everywhere);
    }

    #[test]
    fn places_say_what_is_missing() {
        let title = |route: &Route| EmptyState::of(route).title;
        assert_eq!(title(&Route::Videos(Some(PathBuf::from("/v")))), "This folder is empty");
        assert_eq!(title(&Route::History), "Nothing watched yet");
        assert_eq!(title(&Route::Music(MusicRoute::Albums(None))), "No music yet");
        assert_eq!(title(&Route::Home), "Nothing here yet");
    }
}
