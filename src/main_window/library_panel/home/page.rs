//! Home's page (S4.9.1; `sidebar-home`): each section's heading with its `>`, which lists the
//! section in full, then its first items as the listing draws them: media as cards with their
//! progress sliver, places as rows.
//!
//! Continue Watching says how many of its videos are away (an unplugged drive), and with
//! "Save recently played items" off a note says why only Favorites shows (S4.9 edge cases).
//! The page is short, so it is one scrolling column rather than a virtual list; the keyboard
//! scrolls to its item through [`Page::child_of`](super::Page::child_of). Full library mode
//! shows each section's items as one line of tiles (`player_07`, S4.10.2).

use gpui::{AnyElement, ClickEvent, Context, Div, SharedString, Stateful, div, prelude::*, px};

use super::super::grid::Tile;
use super::super::listing::{Body, Item, Items};
use super::super::{INSET, MainWindow};
use super::{Page, welcome};
use crate::assets::icons;
use crate::db::metadata::MediaKind;
use crate::navigation::route::{HomeSection, Route};
use crate::theme::Palette;
use crate::ui::button::IconButton;

/// A heading's height, the air above every heading but the first, and its text.
const HEADING_HEIGHT: f32 = 42.;
const HEADING_GAP: f32 = 22.;
const HEADING_TEXT: f32 = 17.;
/// Full library mode's heading text and the air above it.
const FULL_HEADING_TEXT: f32 = 20.;
const FULL_HEADING_GAP: f32 = 36.;
/// The `>` button and its icon.
const MORE_BUTTON: f32 = 28.;
const MORE_ICON: f32 = 20.;
/// A song's or a place's square tile in full library mode (`player_07`).
const SQUARE: f32 = 192.;
/// A note's text, and the air below the page.
const NOTE_TEXT: f32 = 13.;
const BOTTOM: f32 = 20.;

/// What the privacy note says.
const NOT_SAVED: &str = "Recently played items aren't saved, so only Favorites shows here. Turn \
                         on “Save recently played items” in Settings to see what you were \
                         watching.";

impl MainWindow {
    /// Home's sections, its welcome while it has none, or nothing while it is read.
    pub(in super::super) fn render_home(&self, p: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(page) = self.home_page() else { return div().into_any_element() };
        if page.sections.is_empty() {
            return welcome::render(page.history, p, cx).into_any_element();
        }
        let column = div()
            .id("library-home")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.library_panel.home_scroll)
            .px(px(INSET))
            .pb(px(BOTTOM))
            .children(self.home_children(page, p, cx));
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(!page.history, |page| page.child(note(NOT_SAVED.into(), p).px(px(INSET))))
            .child(column)
            .into_any_element()
    }

    /// The page's children, one per heading, item and note, as [`Page::child_of`] counts them.
    fn home_children(&self, page: &Page, p: &Palette, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Body::Items(items) = self.listing(cx) else { return Vec::new() };
        let mut children = Vec::new();
        let (mut card, mut row) = (0, page.cards.len());
        let lined = self.library_panel.is_full();
        for (at, shown) in page.sections.iter().enumerate() {
            children.push(heading(shown.section, at, lined, p, cx).into_any_element());
            let indices = (card..card + shown.cards).chain(row..row + shown.rows);
            if lined {
                let tiles = indices.filter_map(|index| self.home_tile(&items, index, p, cx));
                let gap = self.home_tiles().gap;
                let line =
                    div().flex().items_start().gap(px(gap)).children(tiles.collect::<Vec<_>>());
                children.push(line.into_any_element());
            } else {
                children.extend(indices.filter_map(|index| self.home_item(&items, index, p, cx)));
            }
            (card, row) = (card + shown.cards, row + shown.rows);
            if let Some(away) = page.away_note(shown.section) {
                children.push(note(away.into(), p).into_any_element());
            }
        }
        children
    }
}

impl MainWindow {
    /// Item `index` as a tile in full library mode (`player_07`): a video as the grid's, a song
    /// or a place as a square, a song with its artist.
    fn home_tile(
        &self,
        items: &Items<'_>,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let square = Tile::new(index, (SQUARE, SQUARE));
        Some(match items.get(index)? {
            Item::Card(card) if card.kind == MediaKind::Audio => {
                self.render_card_tile(Tile { artist: true, ..square }, card, p, cx)
            }
            Item::Card(card) => {
                self.render_card_tile(Tile::new(index, self.home_tiles().tile()), card, p, cx)
            }
            Item::Row(row) => self.render_row_tile(square, row, p, cx),
        })
    }

    /// Item `index` as the listing draws it: a card, or a row filled when selected.
    fn home_item(
        &self,
        items: &Items<'_>,
        index: usize,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let element = match items.get(index)? {
            Item::Card(listed) => self.render_card(index, listed, p, cx),
            Item::Row(listed) => {
                let selected = self.library_panel.selection.contains(&listed.key());
                listed.render(index, selected, p, cx)
            }
        };
        Some(element.into_any_element())
    }
}

/// Section `at`'s heading and its `>`, which opens the section in full; over a `lined`
/// section larger, with `>` right after its name (`player_07`).
fn heading(
    section: HomeSection,
    at: usize,
    lined: bool,
    p: &Palette,
    cx: &mut Context<MainWindow>,
) -> Stateful<Div> {
    let (text, gap) =
        if lined { (FULL_HEADING_TEXT, FULL_HEADING_GAP) } else { (HEADING_TEXT, HEADING_GAP) };
    let name = div().min_w_0().truncate().child(section.label());
    let more = IconButton::new(
        ("library-home-more", section as usize),
        icons::CHEVRON_RIGHT,
        format!("Show all of {}", section.label()),
    )
    .size(MORE_BUTTON, MORE_ICON)
    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
        this.navigate(Route::HomeSection(section), cx);
    }));
    div()
        .id(("library-home-heading", section as usize))
        .h(px(HEADING_HEIGHT))
        .when(at > 0, |heading| heading.mt(px(gap)))
        .flex()
        .items_center()
        .gap(px(8.))
        .text_size(px(text))
        .text_color(p.text_strong)
        .child(if lined { name } else { name.flex_1() })
        .child(more)
}

/// A muted line under a section or above the page.
fn note(text: SharedString, p: &Palette) -> Div {
    div().py(px(8.)).text_size(px(NOTE_TEXT)).text_color(p.text_muted).child(text)
}
