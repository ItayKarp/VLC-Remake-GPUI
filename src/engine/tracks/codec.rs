//! Short codec names for track labels, from the FourCCs libVLC identifies codecs by.

/// libVLC's FourCCs (`vlc_fourcc.h`) and the names users know them by.
const NAMES: &[(&[u8; 4], &str)] = &[
    (b"a52 ", "AC3"),
    (b"eac3", "E-AC3"),
    (b"dts ", "DTS"),
    (b"trhd", "TrueHD"),
    (b"mlp ", "MLP"),
    (b"mp4a", "AAC"),
    (b"mpga", "MPEG Audio"),
    (b"mp3 ", "MP3"),
    (b"Opus", "Opus"),
    (b"vorb", "Vorbis"),
    (b"flac", "FLAC"),
    (b"alac", "ALAC"),
    (b"araw", "PCM"),
    (b"lpcm", "PCM"),
    (b"s16l", "PCM"),
    (b"s16b", "PCM"),
    (b"s24l", "PCM"),
    (b"s32l", "PCM"),
    (b"f32l", "PCM"),
    (b"h264", "H.264"),
    (b"hevc", "HEVC"),
    (b"av01", "AV1"),
    (b"VP90", "VP9"),
    (b"VP80", "VP8"),
    (b"mp4v", "MPEG-4"),
    (b"mpgv", "MPEG-2"),
    (b"mp2v", "MPEG-2"),
    (b"mp1v", "MPEG-1"),
    (b"WVC1", "VC-1"),
    (b"theo", "Theora"),
    (b"MJPG", "MJPEG"),
    (b"subt", "SRT"),
    (b"ssa ", "ASS"),
    (b"wvtt", "WebVTT"),
    (b"tx3g", "Timed Text"),
    (b"ttml", "TTML"),
    (b"spu ", "VobSub"),
    (b"bdpg", "PGS"),
    (b"dvbs", "DVB"),
    (b"telx", "Teletext"),
    (b"c608", "CC"),
    (b"c708", "CC"),
];

/// The name of the codec with libVLC FourCC `fourcc` ("AC3" for `a52 `). An unlisted one shows
/// as its FourCC in capitals; `None` for 0 or one that isn't text.
pub fn name(fourcc: u32) -> Option<String> {
    // `VLC_FOURCC(a, b, c, d)` puts `a` in the lowest byte.
    let bytes = fourcc.to_le_bytes();
    if let Some((_, name)) = NAMES.iter().find(|(code, _)| **code == bytes) {
        return Some((*name).to_owned());
    }
    let text = std::str::from_utf8(&bytes).ok()?.trim();
    let printable = !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_graphic());
    printable.then(|| text.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fourcc(code: [u8; 4]) -> u32 {
        u32::from_le_bytes(code)
    }

    #[test]
    fn known_codecs_get_their_common_names() {
        assert_eq!(name(fourcc(*b"a52 ")).as_deref(), Some("AC3"));
        assert_eq!(name(fourcc(*b"subt")).as_deref(), Some("SRT"));
        assert_eq!(name(fourcc(*b"h264")).as_deref(), Some("H.264"));
    }

    #[test]
    fn unknown_codecs_show_their_fourcc() {
        assert_eq!(name(fourcc(*b"wma2")).as_deref(), Some("WMA2"));
        assert_eq!(name(0), None);
        assert_eq!(name(fourcc(*b"\x01\x02ab")), None);
    }
}
