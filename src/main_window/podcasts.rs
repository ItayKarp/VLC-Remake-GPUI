//! The main window's side of podcasts (S8.17.2, not designed): Network → Podcasts' "+ Subscribe
//! to Podcast" asks for a feed's address in a dialog, downloads it, and goes to its episodes, or
//! says in a toast why it couldn't be read. An episode that plays to its end is marked played.

use gpui::{App, AppContext as _, Context, Entity, Window, prelude::*};

use super::MainWindow;
use crate::app::media::Media;
use crate::db::podcasts::FeedId;
use crate::navigation::route::Route;
use crate::podcasts;
use crate::ui::modal::{Dialog, DialogButton};
use crate::ui::text_input::TextInput;
use crate::ui::toast::{Level, Toast};

impl MainWindow {
    /// "+ Subscribe to Podcast": asks for the feed's address, then subscribes.
    pub(super) fn ask_podcast_feed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input: Entity<TextInput> = cx.new(|cx| {
            TextInput::new(window, cx)
                .with_placeholder(tr!("e.g., https://www.example.com/podcast.rss"))
                .with_label(tr!("Feed address"))
        });
        let main = cx.entity().downgrade();
        let subscribe = {
            let input = input.clone();
            DialogButton::new(tr!("Subscribe")).primary().on_click(move |_, cx| {
                let typed = input.read(cx).text().to_owned();
                main.update(cx, |main, cx| main.subscribe_podcast(&typed, cx)).ok();
            })
        };
        let body = move |_: &mut Window, _: &mut App| input.clone().into_any_element();
        let buttons = vec![DialogButton::new(tr!("Cancel")), subscribe];
        let dialog = cx.new(|cx| Dialog::new(tr!("Subscribe to Podcast"), body, buttons, cx));
        self.modals.update(cx, |layer, cx| layer.push(dialog, window, cx));
    }

    /// Subscribes to the feed `typed` names and shows its episodes once it is read.
    fn subscribe_podcast(&mut self, typed: &str, cx: &mut Context<Self>) {
        let subscribing = podcasts::subscribe(typed, cx);
        let reading = Toast::new(Level::Info, tr!("Reading the podcast's feed…"));
        let reading = self.toasts.update(cx, |toasts, cx| toasts.push(reading, cx));
        cx.spawn(async move |main, cx| {
            let subscribed = subscribing.await;
            main.update(cx, |main, cx| {
                main.toasts.update(cx, |toasts, cx| toasts.dismiss(reading, cx));
                main.subscribed(subscribed, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Shows the podcast just subscribed to, or why it couldn't be.
    fn subscribed(
        &mut self,
        subscribed: Result<FeedId, podcasts::fetch::Failure>,
        cx: &mut Context<Self>,
    ) {
        match subscribed {
            Ok(id) => self.navigate(Route::Podcast(id), cx),
            Err(failure) => self.show_toast(Toast::new(Level::Error, failure.message()), cx),
        }
    }

    /// What played to its end is marked played, if it is a podcast episode.
    pub(super) fn episode_played(&mut self, cx: &mut Context<Self>) {
        if let Some(Media::Url(url)) = self.player_media(cx) {
            podcasts::played_to_end(&url, cx);
        }
    }
}
