//! Forgetting what VLC remembers (S1.7.5, S5.5.1): the playback history, Open Recent and Recent
//! Streams at once, from the Clear History command or Settings → Privacy's "Clear history now",
//! which first asks with how much there is to forget.

use gpui::{App, PromptLevel, Window};

use crate::db::media;
use crate::history::History;
use crate::network;
use crate::opening::recent;
use crate::settings;

/// How much history there is to forget.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Media with a play, a position or a watched mark.
    pub media: u32,
    /// Items in Open Recent.
    pub recent: usize,
    /// Recent network streams.
    pub streams: usize,
}

impl Counts {
    /// Whether there is nothing to forget.
    pub fn is_empty(self) -> bool {
        self == Self::default()
    }

    /// What the confirmation says will be forgotten.
    pub fn describe(self) -> String {
        let mut listed = Vec::new();
        if self.media > 0 {
            listed.push(trn!("{count} played item", "{count} played items", self.media));
        }
        if self.recent > 0 {
            let items = self.recent;
            listed.push(trn!("{count} item in Open Recent", "{count} items in Open Recent", items));
        }
        if self.streams > 0 {
            listed.push(trn!("{count} recent stream", "{count} recent streams", self.streams));
        }
        match listed.as_slice() {
            [] => String::new(),
            [only] => only.clone(),
            [first @ .., last] => tr!("{list} and {last}", list = first.join(tr!(", ")), last),
        }
    }
}

/// Forgets the playback history, the recent media list and the recent network streams. The
/// library, playlists and favorites stay.
pub fn clear_history(cx: &mut App) {
    if let Some(history) = History::get(cx) {
        history.clear();
    }
    recent::clear(cx);
    network::clear_recent(cx);
}

/// "Clear history now": asks first, saying what will be forgotten, unless confirmations are off
/// (S5.3.3); says so when there is nothing to forget.
pub fn ask_to_clear(window: &mut Window, cx: &mut App) {
    if !settings::get(cx).prefs.startup.show_dialogs {
        clear_history(cx);
        return;
    }
    let media = History::get(cx).map(|history| history.query("history", media::count_with_history));
    let recent = recent::stored_count(cx);
    let streams = network::recent_count(cx);
    window
        .spawn(cx, async move |cx| {
            let media = match media {
                Some(answer) => answer.await.ok().flatten().unwrap_or_default(),
                None => 0,
            };
            let counts = Counts { media, recent, streams };
            cx.update(|window, cx| confirm(counts, window, cx)).ok();
        })
        .detach();
}

/// Shows the confirmation for `counts`, and clears on Clear History.
fn confirm(counts: Counts, window: &mut Window, cx: &mut App) {
    if counts.is_empty() {
        let detail = tr!("Nothing has been played or opened since the history was last cleared.");
        let title = tr!("No history to clear");
        // Nothing waits for the answer: OK only closes the alert.
        drop(window.prompt(PromptLevel::Info, title, Some(detail), &[tr!("OK")], cx));
        return;
    }
    let detail = tr!(
        "VLC forgets {forgotten}. Your library, playlists and favorites stay. This can't be \
         undone.",
        forgotten = counts.describe()
    );
    let answer = window.prompt(
        PromptLevel::Warning,
        tr!("Clear your history?"),
        Some(&detail),
        &[tr!("Clear History"), tr!("Cancel")],
        cx,
    );
    window
        .spawn(cx, async move |cx| {
            if answer.await == Ok(0) {
                cx.update(|_, cx| clear_history(cx)).ok();
            }
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_read_as_a_sentence() {
        assert_eq!(Counts::default().describe(), "");
        let one = Counts { media: 1, ..Counts::default() };
        assert_eq!(one.describe(), "1 played item");
        let two = Counts { media: 42, streams: 1, ..Counts::default() };
        assert_eq!(two.describe(), "42 played items and 1 recent stream");
        let all = Counts { media: 2, recent: 10, streams: 3 };
        assert_eq!(all.describe(), "2 played items, 10 items in Open Recent and 3 recent streams");
    }
}
