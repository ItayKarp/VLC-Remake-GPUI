//! The main window's side of updates (S9.5): it tells the user once about an update readied to
//! install at quit, a newer version the updater can't install here, or an update rolled back,
//! with the release notes and Skip This Version a click away.

use gpui::{Context, Subscription};

use super::MainWindow;
use crate::ui::toast::{Level, Toast};
use crate::updates::{self, News, Offer, Updater};

/// Shows the updater's news as it comes.
#[derive(Default)]
pub(super) struct UpdateNews {
    _news: Option<Subscription>,
}

impl MainWindow {
    /// Shows the updater's news, now and whenever it has more.
    pub(super) fn set_up_updates(&mut self, cx: &mut Context<Self>) {
        self.show_update_news(cx);
        let news = cx.observe_global::<Updater>(Self::show_update_news);
        self.update_news = UpdateNews { _news: Some(news) };
    }

    fn show_update_news(&mut self, cx: &mut Context<Self>) {
        let Some(news) = updates::take_news(cx) else { return };
        let toast = match news {
            News::Ready(offer) => offer_toast(
                tr!("VLC {version} is ready. It installs when you quit.", version = offer.version),
                offer,
            ),
            News::Available(offer) => {
                offer_toast(tr!("VLC {version} is available.", version = offer.version), offer)
            }
            News::RolledBack(version) => Toast::new(
                Level::Warning,
                tr!("VLC {version} didn't start, so this version was put back.", version),
            ),
        };
        self.show_toast(toast, cx);
    }
}

/// A toast saying `message` about `offer`, with its release notes and a way to skip it. It stays
/// until dismissed, as the user may want to act on it, and it may arrive while nobody looks.
fn offer_toast(message: String, offer: Offer) -> Toast {
    let Offer { version, notes_url } = offer;
    Toast::new(Level::Warning, message)
        .action(tr!("What's New"), move |_, cx| cx.open_url(notes_url.as_str()))
        .action(tr!("Skip This Version"), move |_, cx| updates::skip(&version, cx))
}
