//! What the listing shows when a place lists nothing (S4.7.6). Not designed: a muted icon, a
//! line saying what is missing, a hint at how to fill it, and, for a search of one section, a
//! button that searches everywhere instead.
//!
//! A search says "No results for “…”", a folder "This folder is empty", History "Nothing watched
//! yet", Albums, Artists and Genres that no songs have been read, Playlists that none were
//! made, and each of Home's sections in full what fills it (S4.9). Anything else says "Nothing
//! here yet." Home itself has its own welcome.

use gpui::{ClickEvent, Context, Div, SharedString, div, prelude::*, px};

use super::{INSET, MainWindow};
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::navigation::route::{HomeSection, MusicRoute, NetworkCategory, Route, Search, Section};
use crate::theme::Palette;
use crate::theme::text_px;
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
            Route::HomeSection(section) => home_section(*section),
            Route::Videos(Some(_)) => state(
                icons::FOLDER,
                tr!("This folder is empty"),
                tr!("It holds no videos or folders."),
            ),
            Route::Music(MusicRoute::Songs(Some(_))) => state(
                icons::FOLDER,
                tr!("This folder is empty"),
                tr!("It holds no songs or folders."),
            ),
            Route::Music(
                MusicRoute::Albums(None) | MusicRoute::Artists(None) | MusicRoute::Genres(None),
            ) => state(icons::MUSIC, tr!("No music yet"), crate::i18n::text(NO_SONGS)),
            Route::History => state(
                icons::RECENT,
                tr!("Nothing watched yet"),
                tr!("What you play shows up here."),
            ),
            Route::Network(Some(category)) => network_category(*category),
            Route::Podcast(_) => state(
                icons::STREAMING,
                tr!("No episodes"),
                tr!("Its feed lists no episodes to play yet."),
            ),
            Route::Playlists(None) => state(
                icons::PLAYLIST,
                tr!("No playlists yet"),
                tr!("Save the queue as a playlist to see it here."),
            ),
            _ => state(icons::FOLDER, tr!("Nothing here yet"), ""),
        }
    }
}

/// What an empty Network category says.
fn network_category(category: NetworkCategory) -> EmptyState {
    let (title, hint) = match category {
        NetworkCategory::Recent => (
            tr!("No recent streams"),
            tr!("Streams you open with Open Network Stream show up here."),
        ),
        NetworkCategory::Podcasts => (
            tr!("No podcasts yet"),
            tr!("Choose Subscribe to Podcast and enter a podcast's feed address."),
        ),
        NetworkCategory::Radio => (
            tr!("No radio stations yet"),
            tr!("Right-click a recent stream and choose Add to Radio."),
        ),
    };
    EmptyState { icon: icons::STREAMING, title: title.to_owned(), hint, everywhere: false }
}

/// The hint of Music's empty categories.
const NO_SONGS: &str =
    tr_noop!("Songs in your music folders show up here once the library has read them.");

/// What a Home section's full list says while it is empty.
fn home_section(section: HomeSection) -> EmptyState {
    let (icon, title, hint) = match section {
        HomeSection::ContinueWatching => (
            icons::VIDEOS,
            tr!("Nothing to continue"),
            tr!("Videos you leave partway show up here."),
        ),
        HomeSection::ListenAgain => {
            (icons::MUSIC, tr!("Nothing played yet"), tr!("Songs you play show up here."))
        }
        HomeSection::Favorites => (
            icons::FAVORITE,
            tr!("No favorites yet"),
            tr!("Choose Favorite in a file's or a playlist's menu to see it here."),
        ),
        HomeSection::Folders => (
            icons::FOLDER,
            tr!("No folders visited yet"),
            tr!("The folders you open most show up here."),
        ),
        HomeSection::Playlists => (
            icons::PLAYLIST,
            tr!("No playlists visited yet"),
            tr!("The playlists you open most show up here."),
        ),
        HomeSection::Artists => (
            icons::ARTIST,
            tr!("No artists played yet"),
            tr!("The artists you play most show up here."),
        ),
    };
    EmptyState { icon, title: title.to_owned(), hint, everywhere: false }
}

fn searched(search: &Search) -> EmptyState {
    let everywhere = search.scope == Section::Home;
    EmptyState {
        icon: icons::SEARCH,
        title: tr!("No results for “{query}”", query = search.query),
        hint: if everywhere {
            tr!("Check the spelling, or try fewer words.")
        } else {
            tr!("Try fewer words, or search the whole library.")
        },
        everywhere: !everywhere,
    }
}

impl MainWindow {
    /// The block saying why the current place lists nothing.
    pub(super) fn render_empty(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let state = EmptyState::of(self.library_panel.history.current());
        let title = div()
            .text_size(text_px(TITLE_TEXT))
            .text_color(p.text)
            .child(SharedString::from(state.title));
        let hint = (!state.hint.is_empty()).then(|| {
            div().text_size(text_px(HINT_TEXT)).text_color(p.text_muted).child(state.hint)
        });
        let everywhere = state.everywhere.then(|| {
            Button::new("library-empty-everywhere", tr!("Search Everywhere")).on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.search_everywhere(cx);
                },
            ))
        });
        div()
            .pt(px(TOP))
            .px(px(INSET))
            .flex_row_start()
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
        assert_eq!(title(&Route::HomeSection(HomeSection::Favorites)), "No favorites yet");
    }
}
