//! The icon rail (S4.5.4): Home, Videos, Music, Network, Playlists, History and "+", each icon
//! an outline, filled orange at its section's root, or filled grey deeper in it. Holding a drag
//! of the library's media over an icon shows its section ([`drag`](super::drag)).

use gpui::{ClickEvent, Context, Div, Hsla, div, prelude::*, px};

use super::MainWindow;
use crate::assets::icons;
use crate::i18n::DirectionalStyle;
use crate::navigation::rail::{self, RailItem, RailState};
use crate::navigation::route::Section;
use crate::theme::Palette;
use crate::ui::button::IconButton;

/// A rail button's side and its icon's size; the buttons' centers are 46 px apart, the first 24
/// px below the rail's top (`player_02`).
const BUTTON: f32 = 36.;
const ICON: f32 = 20.;
const SPACING: f32 = 46.;
const FIRST_CENTER: f32 = 24.;

impl MainWindow {
    /// The rail down the sidebar's left edge.
    pub(super) fn render_rail(&self, p: &Palette, cx: &mut Context<Self>) -> Div {
        let route = self.library_panel.history.current();
        let buttons = RailItem::ALL.map(|item| {
            let state = rail::state(item, route);
            match item {
                RailItem::Section(section) => section_button(section, state, p, cx),
                RailItem::Add => self.add_button(cx),
            }
        });
        div()
            .w(px(super::RAIL_WIDTH))
            .h_full()
            .flex_row_start()
            .flex_none()
            .flex_col()
            .items_center()
            .gap(px(SPACING - BUTTON))
            .pt(px(FIRST_CENTER - BUTTON / 2.))
            .bg(p.sidebar)
            .children(buttons)
    }

    /// "+": adds media (S4.5.7).
    fn add_button(&self, cx: &mut Context<Self>) -> Div {
        let button = IconButton::new("rail-add", icons::ADD, tr!("Add media"))
            .size(BUTTON, ICON)
            .selected(self.library_panel.add_menu.is_some())
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.toggle_add_menu(window, cx);
            }));
        div().relative().child(self.library_panel.add_button.element()).child(button)
    }
}

fn section_button(
    section: Section,
    state: RailState,
    p: &Palette,
    cx: &mut Context<MainWindow>,
) -> Div {
    let (outline, filled) = section_icons(section);
    let (icon, color): (_, Hsla) = match state {
        RailState::Outline => (outline, p.icon),
        RailState::Root => (filled, p.accent_fg),
        RailState::Inside => (filled, p.icon),
    };
    let button = IconButton::new(("rail", section as usize), icon, section.label())
        .size(BUTTON, ICON)
        .color(color)
        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
            this.navigate(section.root(), cx);
        }));
    super::drag::spring_loaded(div().child(button), section, cx)
}

/// A section's outline and filled icons. History has no filled icon in the set, so its outline
/// is colored instead.
pub(super) fn section_icons(section: Section) -> (&'static str, &'static str) {
    match section {
        Section::Home => (icons::HOME, icons::HOME_FILLED),
        Section::Videos => (icons::VIDEOS, icons::VIDEOS_FILLED),
        Section::Music => (icons::SONGS, icons::SONGS_FILLED),
        Section::Network => (icons::STREAMING, icons::STREAMING_FILLED),
        Section::Playlists => (icons::PLAYLIST, icons::PLAYLIST_FILLED),
        Section::History => (icons::RECENT, icons::RECENT),
    }
}
