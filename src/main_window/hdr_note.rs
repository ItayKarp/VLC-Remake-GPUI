//! What the main window says about HDR video (S7.5): a line under the settings panel's HDR row
//! naming what plays and how it is shown, and a message over the video, once per media, when it
//! shows with wrong colors (HDR left washed out, or Dolby Vision profile 5).

use gpui::{Context, Window};

use super::MainWindow;
use crate::engine::hdr::{HdrStatus, SourceColour, ToneMap, Transfer};
use crate::native_window::{self, DisplayRange};

/// Why HDR shows washed out with HDR Off.
const WASHED_OUT: &str =
    tr_noop!("HDR video without tone mapping: colors look washed out. Set HDR to Auto.");
/// Why Dolby Vision profile 5 shows purple and green.
const DOLBY_VISION_5: &str = tr_noop!(
    "Dolby Vision without an HDR10 base: VLC can't convert its colors, so they look wrong."
);
/// What an HDR display gets.
const HDR_DISPLAY: &str = tr_noop!(
    "This display can show HDR, but VLC tone-maps it to SDR for now: its video path is SDR only."
);

impl MainWindow {
    /// The line under the HDR row for the video playing and the display the window is on;
    /// `None` when there's nothing to say (SDR video, or none).
    pub(super) fn hdr_note(&self, window: &Window, cx: &Context<Self>) -> Option<String> {
        let (player, _) = self.player.as_ref()?;
        describe(player.read(cx).hdr(), native_window::display_range(window))
    }

    /// Says over the video, as its status first reports it, when the video shows with wrong
    /// colors.
    pub(super) fn warn_hdr(&mut self, status: HdrStatus, cx: &mut Context<Self>) {
        if let Some(warning) = warning(status) {
            self.show_osd(warning, cx);
        }
    }
}

/// What to say about `status` on a display that can show `range`.
fn describe(status: HdrStatus, range: DisplayRange) -> Option<String> {
    let video = video_note(status)?;
    Some(if range == DisplayRange::Hdr {
        format!("{video} {}", crate::i18n::text(HDR_DISPLAY))
    } else {
        video
    })
}

/// What the video is and how it shows; `None` for SDR video.
fn video_note(status: HdrStatus) -> Option<String> {
    let source = status.source.unwrap_or_default();
    if let Some(warning) = warning(status) {
        return Some(warning.to_owned());
    }
    let map = status.tone_map?;
    let name = name(map.transfer);
    Some(match peak(map, source) {
        _ if source.transfer != Some(map.transfer) => {
            tr!("Read as {name} and tone-mapped to SDR.", name)
        }
        Some(peak) => tr!("{name} video, tone-mapped to SDR from {peak} nits.", name, peak),
        None => tr!("{name} video, tone-mapped to SDR.", name),
    })
}

/// The warning for video that shows with wrong colors.
fn warning(status: HdrStatus) -> Option<&'static str> {
    let source = status.source?;
    if source.is_unshowable_dolby_vision() {
        Some(crate::i18n::text(DOLBY_VISION_5))
    } else {
        let washed_out = source.transfer.is_some() && status.tone_map.is_none();
        washed_out.then(|| crate::i18n::text(WASHED_OUT))
    }
}

/// The peak brightness in nits ("1000") of HDR10 whose metadata says it.
fn peak(map: ToneMap, source: SourceColour) -> Option<String> {
    match (map.transfer, source.peak) {
        (Transfer::Pq, Some(_)) => Some(format!("{:.0}", map.peak)),
        _ => None,
    }
}

fn name(transfer: Transfer) -> &'static str {
    match transfer {
        Transfer::Pq => "HDR10",
        Transfer::Hlg => "HLG",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HDR10: SourceColour =
        SourceColour { transfer: Some(Transfer::Pq), peak: Some(1000.), dolby_vision: false };
    const PQ: ToneMap = ToneMap { transfer: Transfer::Pq, peak: 1000. };

    #[test]
    fn sdr_video_says_nothing() {
        let sdr = HdrStatus { source: Some(SourceColour::default()), tone_map: None };
        assert_eq!(describe(sdr, DisplayRange::Hdr), None);
        assert_eq!(describe(HdrStatus::default(), DisplayRange::Sdr), None);
    }

    #[test]
    fn mapped_video_names_its_format_and_peak() {
        let status = HdrStatus { source: Some(HDR10), tone_map: Some(PQ) };
        let note = describe(status, DisplayRange::Sdr);
        assert_eq!(note.as_deref(), Some("HDR10 video, tone-mapped to SDR from 1000 nits."));
        let hdr_display = describe(status, DisplayRange::Hdr).unwrap_or_default();
        assert!(hdr_display.ends_with(HDR_DISPLAY));
        let forced = HdrStatus { source: None, tone_map: Some(PQ) };
        assert_eq!(video_note(forced).as_deref(), Some("Read as HDR10 and tone-mapped to SDR."));
    }

    #[test]
    fn wrong_colors_warn() {
        let off = HdrStatus { source: Some(HDR10), tone_map: None };
        assert_eq!(warning(off), Some(WASHED_OUT));
        let profile5 = SourceColour { dolby_vision: true, ..SourceColour::default() };
        let status = HdrStatus { source: Some(profile5), tone_map: None };
        assert_eq!(warning(status), Some(DOLBY_VISION_5));
        assert_eq!(warning(HdrStatus { source: Some(HDR10), tone_map: Some(PQ) }), None);
    }
}
