//! A card's menu, from its `•••` or a right-click (S4.6.3, S4.6.6): play, play next, add to the
//! queue or to a playlist, and mark watched or unwatched. On a card in a selection of several it
//! acts on every selected card, in list order; on any other card it selects that card alone
//! first. The full file menu, with Properties, Show in Folder and the rest, is S4.8.1's.

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, Pixels, Point, Subscription, WeakEntity, Window,
};

use super::MainWindow;
use super::listing::{Body, Item, card_key};
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::assets::icons;
use crate::history::History;
use crate::playlists;
use crate::ui::menu::{ContextMenu, MenuItem};
use crate::ui::popover;

/// The open card menu, where it opened, and the subscription that forgets it once dismissed.
pub(super) type CardMenu = Option<(Entity<ContextMenu>, Point<Pixels>, Subscription)>;

/// What the menu acts on: the cards' media, in list order, and whether all of them are watched.
#[derive(Clone)]
struct Targets {
    media: Vec<Media>,
    all_watched: bool,
    /// The listing index of the one card, when there is one: Play plays its folder from it.
    single: Option<usize>,
}

impl MainWindow {
    /// Opens the menu of the card at item `index` (and the cards selected with it) at
    /// `position`.
    pub(super) fn open_card_menu(
        &mut self,
        index: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(targets) = self.card_targets(index, cx) else { return };
        let menu_items = menu_items(&cx.entity().downgrade(), &targets, cx);
        let menu = ContextMenu::build(menu_items, window, cx);
        let subscription = cx.subscribe(&menu, |this, _, _: &DismissEvent, cx| {
            this.library_panel.card_menu = None;
            cx.notify();
        });
        self.library_panel.card_menu = Some((menu, position, subscription));
        cx.notify();
    }

    /// The cards the menu of item `index` acts on, selecting that card alone when it isn't
    /// selected.
    fn card_targets(&mut self, index: usize, cx: &mut Context<Self>) -> Option<Targets> {
        let Body::Items(items) = self.listing(cx) else { return None };
        let Some(Item::Card(card)) = items.get(index) else { return None };
        let key = card_key(card);
        if !self.library_panel.selection.contains(&key) {
            self.library_panel.selection.select(key);
        }
        let Body::Items(items) = self.listing(cx) else { return None };
        let keys = items.keys();
        let cards: Vec<_> = self
            .library_panel
            .selection
            .indices(&keys)
            .into_iter()
            .filter_map(|at| match items.get(at) {
                Some(Item::Card(card)) => Some(card),
                _ => None,
            })
            .collect();
        Some(Targets {
            media: cards.iter().map(|card| Media::File(card.path.clone())).collect(),
            all_watched: cards.iter().all(|card| card.watch.is_watched()),
            single: (cards.len() == 1).then_some(index),
        })
    }

    /// Marks `media` watched or unwatched, then lists its folder again to show it.
    fn mark_cards_watched(&mut self, media: Vec<Media>, watched: bool, cx: &mut Context<Self>) {
        let Some(history) = History::get(cx) else { return };
        for media in media {
            history.set_watched(media, watched);
        }
        // The history thread runs jobs in order, so the folder reads after the change.
        self.reload_folder(cx);
    }
}

fn menu_items(main: &WeakEntity<MainWindow>, targets: &Targets, cx: &App) -> Vec<MenuItem> {
    let placed = |label: &'static str, placement: Placement| {
        let (main, media) = (main.clone(), targets.media.clone());
        MenuItem::action(label, move |window, cx| {
            let request = OpenRequest::placed(media.clone(), placement);
            main.update(cx, |main, cx| main.open(request, window, cx)).ok();
        })
    };
    let (play, mark) = (main.clone(), main.clone());
    let (to_play, to_mark) = (targets.clone(), targets.media.clone());
    let watched = !targets.all_watched;
    let watched_label = if watched { "Mark as Watched" } else { "Mark as Unwatched" };
    vec![
        MenuItem::action("Play", move |window, cx| {
            let targets = to_play.clone();
            play.update(cx, |main, cx| match targets.single {
                Some(index) => main.open_item(index, window, cx),
                None => main.play_cards(targets.media, 0, window, cx),
            })
            .ok();
        })
        .icon(icons::PLAY),
        placed("Play Next", Placement::PlayNext),
        placed("Add to Queue", Placement::Enqueue).icon(icons::QUEUE),
        add_to_playlist(main, &targets.media, cx),
        MenuItem::Separator,
        MenuItem::action(watched_label, move |_, cx| {
            let media = to_mark.clone();
            mark.update(cx, |main, cx| main.mark_cards_watched(media, watched, cx)).ok();
        })
        .icon(icons::CHECK),
    ]
}

/// Add to Playlist ▸: the saved playlists, then New Playlist… holding `media`.
fn add_to_playlist(main: &WeakEntity<MainWindow>, media: &[Media], cx: &App) -> MenuItem {
    let mut items: Vec<MenuItem> = playlists::summaries(cx)
        .iter()
        .map(|summary| {
            let (main, summary, media) = (main.clone(), summary.clone(), media.to_vec());
            MenuItem::action(summary.title.clone(), move |window, cx| {
                main.update(cx, |_, cx| MainWindow::add_to_playlist(&summary, &media, window, cx))
                    .ok();
            })
        })
        .collect();
    if !items.is_empty() {
        items.push(MenuItem::Separator);
    }
    let (main, files) = (main.clone(), media.to_vec());
    items.push(MenuItem::action("New Playlist…", move |window, cx| {
        let files = files.clone();
        main.update(cx, |main, cx| main.show_new_playlist(files, window, cx)).ok();
    }));
    MenuItem::submenu("Add to Playlist", items).icon(icons::PLAYLIST)
}

/// The card menu at the pointer, while open.
pub(super) fn render(menu: &CardMenu) -> Option<AnyElement> {
    let (menu, position, _) = menu.as_ref()?;
    Some(popover::at_position(*position, 1, menu.clone()))
}
