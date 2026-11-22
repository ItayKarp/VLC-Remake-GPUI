//! A network stream row's menu (S4.11, not designed): Play, Add to Queue, Add to Radio, Copy
//! Link and Remove from its category. Several streams play one after another.

use gpui::{ClipboardItem, Context, Window};

use super::MainWindow;
use super::item_menu::{on_all, on_one};
use crate::app::media::Media;
use crate::app::request::{OpenRequest, Placement};
use crate::assets::icons;
use crate::navigation::route::NetworkCategory;
use crate::network;
use crate::ui::menu::MenuItem;

/// One stream a menu acts on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct StreamTarget {
    /// Its URL.
    pub(super) url: String,
    /// The category it is listed in.
    pub(super) category: NetworkCategory,
}

/// The menu of `streams`, all listed in one category.
pub(super) fn items(
    main: &gpui::WeakEntity<MainWindow>,
    streams: &[StreamTarget],
) -> Vec<MenuItem> {
    let Some(category) = streams.first().map(|stream| stream.category) else { return Vec::new() };
    let play = |label, icon, placement| {
        on_all(main, streams, label, icon, move |main, streams, window, cx| {
            play(main, streams, placement, window, cx);
        })
    };
    let add_to_radio =
        on_all(main, streams, "Add to Radio", icons::STREAMING, |_, streams, _, cx| {
            for stream in streams {
                network::add(&stream.url, NetworkCategory::Radio, cx);
            }
        });
    let copy = on_one(main, streams, "Copy Link", icons::LIST, |_, stream, _, cx| {
        cx.write_to_clipboard(ClipboardItem::new_string(stream.url.clone()));
    });
    let remove =
        on_all(main, streams, remove_label(category), icons::REMOVE, |_, streams, _, cx| {
            for stream in streams {
                network::remove(&stream.url, stream.category, cx);
            }
        });
    let mut items = vec![
        play("Play", icons::PLAY, Placement::Replace),
        play("Add to Queue", icons::QUEUE, Placement::Enqueue),
    ];
    items.extend((category != NetworkCategory::Radio).then_some(add_to_radio));
    items.extend([copy, MenuItem::Separator, remove]);
    items
}

/// Plays `streams` as `placement` says; recent ones move to the top of their list.
fn play(
    main: &mut MainWindow,
    streams: &[StreamTarget],
    placement: Placement,
    window: &mut Window,
    cx: &mut Context<MainWindow>,
) {
    let recent: Vec<String> = streams
        .iter()
        .filter(|stream| stream.category == NetworkCategory::Recent)
        .map(|stream| stream.url.clone())
        .collect();
    network::record_recent(&recent, cx);
    let items = streams.iter().map(|stream| Media::Url(stream.url.clone())).collect();
    main.open(OpenRequest::placed(items, placement), window, cx);
}

/// What Remove says for streams in `category`.
fn remove_label(category: NetworkCategory) -> &'static str {
    match category {
        NetworkCategory::Recent => "Remove from Recent Streams",
        NetworkCategory::Podcasts => "Remove from Podcasts",
        NetworkCategory::Radio => "Remove from Radio",
    }
}
