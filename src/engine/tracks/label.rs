//! Display names for tracks: the container's title, or one built from what is known about the
//! track ("English (Forced) — SRT", "Track 2 — AAC Stereo").

use super::Track;
use crate::engine::language::Language;

/// What to call `track`, the `number`th of its kind (from 1).
pub(super) fn label(track: &Track, number: usize) -> String {
    let base = track.title.clone().unwrap_or_else(|| untitled(track, number));
    match details(track) {
        // A title often names the codec already ("English DTS-HD 5.1").
        Some(details) if !mentions_codec(&base, track) => {
            tr!("{base} — {details}", base, details)
        }
        _ => base,
    }
}

/// The name of a track the container didn't title: its language, or its number.
fn untitled(track: &Track, number: usize) -> String {
    let name =
        track.language.as_ref().map_or_else(|| tr!("Track {number}", number), Language::name);
    if track.forced { tr!("{name} (Forced)", name) } else { name }
}

/// The codec and, for audio, the channel layout ("AC3 5.1").
fn details(track: &Track) -> Option<String> {
    let parts: Vec<String> =
        [track.codec.clone(), track.channels.map(layout)].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn mentions_codec(title: &str, track: &Track) -> bool {
    track.codec.as_ref().is_some_and(|codec| title.to_lowercase().contains(&codec.to_lowercase()))
}

/// How a channel count is usually written.
fn layout(channels: u32) -> String {
    match channels {
        1 => tr!("Mono").to_owned(),
        2 => tr!("Stereo").to_owned(),
        3 => "2.1".to_owned(),
        4 => "4.0".to_owned(),
        5 => "5.0".to_owned(),
        6 => "5.1".to_owned(),
        7 => "6.1".to_owned(),
        8 => "7.1".to_owned(),
        _ => tr!("{channels} ch", channels),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::tracks::{TrackId, TrackKind};

    fn audio(language: &str, codec: &str, channels: u32) -> Track {
        Track {
            language: Language::parse(language),
            codec: Some(codec.into()),
            channels: Some(channels),
            ..Track::new(TrackId(1), TrackKind::Audio)
        }
    }

    #[test]
    fn untitled_tracks_are_named_from_their_details() {
        assert_eq!(label(&audio("eng", "AC3", 6), 1), "English — AC3 5.1");
        assert_eq!(label(&audio("und", "AAC", 2), 3), "Track 3 — AAC Stereo");
        let forced = Track {
            language: Language::parse("fr"),
            forced: true,
            ..Track::new(TrackId(4), TrackKind::Subtitle)
        };
        assert_eq!(label(&forced, 1), "French (Forced)");
    }

    #[test]
    fn titles_win_and_keep_missing_details() {
        let titled = Track { title: Some("Commentary".into()), ..audio("en", "AAC", 2) };
        assert_eq!(label(&titled, 2), "Commentary — AAC Stereo");
        let full = Track { title: Some("English AC3 5.1".into()), ..audio("en", "AC3", 6) };
        assert_eq!(label(&full, 1), "English AC3 5.1");
    }
}
