//! The recent media list behind Open Recent (S1.3.5).
//!
//! What the user opened (a file, a folder, a playlist or a URL, as they picked it) is kept
//! newest first in `local.toml`, because paths belong to this machine. The list keeps at most
//! `privacy.recent_media_count` items. With "Save recently played items" off
//! (`privacy.save_recently_played`), nothing is added, the menu shows nothing, and the OS
//! recent documents list (`app::recent`) isn't fed either.

use gpui::App;

use crate::app;
use crate::app::media::Media;
use crate::network::links;
use crate::privacy::exclusions::Exclusions;
use crate::settings;

/// The most items the list keeps, whatever the setting says.
pub const MAX_COUNT: u32 = 100;

/// Remembers `opened`, in the order opened, if the privacy setting allows it. Links with a
/// user name or password are left out: settings files aren't a place for passwords. So is what
/// "Never remember items matching" names.
pub fn record(opened: &[Media], cx: &mut App) {
    let exclusions = Exclusions::from_settings(cx);
    let privacy = &settings::get(cx).prefs.privacy;
    let remembered = |media: &&Media| !has_credentials(media) && !exclusions.excludes(media);
    let opened: Vec<Media> = opened.iter().filter(remembered).cloned().collect();
    if !privacy.save_recently_played || opened.is_empty() {
        return;
    }
    let count = limit(privacy.recent_media_count);
    settings::update(cx, |s| remember(&mut s.local.recent_media, &opened, count));
    app::recent::add(&opened, cx);
}

/// The recent items to show, newest first; none while history is off.
pub fn items(cx: &App) -> Vec<Media> {
    let settings = settings::get(cx);
    let privacy = &settings.prefs.privacy;
    if !privacy.save_recently_played {
        return Vec::new();
    }
    let stored = settings.local.recent_media.iter();
    stored
        .filter_map(|text| Media::from_stored(text))
        .take(limit(privacy.recent_media_count))
        .collect()
}

/// How many items are kept, shown or not (Clear History counts them while history is off too).
pub fn stored_count(cx: &App) -> usize {
    settings::get(cx).local.recent_media.len()
}

/// Forgets every recent item.
pub fn clear(cx: &mut App) {
    settings::update(cx, |s| s.local.recent_media.clear());
}

/// Whether `media` is a link carrying a user name or password.
fn has_credentials(media: &Media) -> bool {
    matches!(media, Media::Url(url) if links::has_credentials(url))
}

fn limit(count: u32) -> usize {
    count.min(MAX_COUNT) as usize
}

/// Moves each of `opened` to the front of `list`, the last one first, and trims the list to
/// `count` items.
fn remember(list: &mut Vec<String>, opened: &[Media], count: usize) {
    for media in opened {
        let stored = media.to_stored();
        list.retain(|item| *item != stored);
        list.insert(0, stored);
    }
    list.truncate(count);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> Media {
        Media::File(std::env::temp_dir().join(name))
    }

    #[test]
    fn newest_first_without_duplicates() {
        let mut list = Vec::new();
        remember(&mut list, &[file("a.mkv"), file("b.mkv")], 10);
        remember(&mut list, &[Media::Url("https://x/live".into()), file("a.mkv")], 10);
        let decoded: Vec<Media> = list.iter().filter_map(|t| Media::from_stored(t)).collect();
        assert_eq!(decoded, [file("a.mkv"), Media::Url("https://x/live".into()), file("b.mkv")]);
    }

    #[test]
    fn trims_to_the_count() {
        let mut list = Vec::new();
        remember(&mut list, &[file("a"), file("b"), file("c")], 2);
        assert_eq!(list.len(), 2);
        remember(&mut list, &[file("d")], 0);
        assert_eq!(list, Vec::<String>::new());
    }
}
