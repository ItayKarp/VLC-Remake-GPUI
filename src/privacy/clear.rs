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
        let parts = [
            (u64::from(self.media), "played item", "played items"),
            (self.recent as u64, "item in Open Recent", "items in Open Recent"),
            (self.streams as u64, "recent stream", "recent streams"),
        ];
        let listed: Vec<String> = parts
            .iter()
            .filter(|(count, ..)| *count > 0)
            .map(|&(count, one, many)| format!("{count} {}", if count == 1 { one } else { many }))
            .collect();
        match listed.as_slice() {
            [] => String::new(),
            [only] => only.clone(),
            [first @ .., last] => format!("{} and {last}", first.join(", ")),
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
        let detail = "Nothing has been played or opened since the history was last cleared.";
        // Nothing waits for the answer: OK only closes the alert.
        drop(window.prompt(PromptLevel::Info, "No history to clear", Some(detail), &["OK"], cx));
        return;
    }
    let detail = format!(
        "VLC forgets {}. Your library, playlists and favorites stay. This can't be undone.",
        counts.describe()
    );
    let answer = window.prompt(
        PromptLevel::Warning,
        "Clear your history?",
        Some(&detail),
        &["Clear History", "Cancel"],
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
