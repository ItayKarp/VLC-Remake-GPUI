//! The formats media converts to (S8.10.1), each a container and codecs from the encoders libVLC
//! ships (x264, x265, VP8, Theora, AAC, MP3, Vorbis, FLAC), and the libVLC options that make it.

use std::path::Path;

/// A format to convert to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    /// H.264 video and AAC audio in MP4: plays nearly everywhere.
    #[default]
    Mp4H264,
    /// H.265 video and AAC audio in MP4: smaller, slower to make.
    Mp4H265,
    /// VP8 video and Vorbis audio in WebM.
    WebM,
    /// Theora video and Vorbis audio in Ogg.
    Ogg,
    /// MP3 audio.
    Mp3,
    /// AAC audio in M4A.
    Aac,
    /// Vorbis audio in Ogg.
    Vorbis,
    /// FLAC audio, losslessly.
    Flac,
}

/// The bit rate audio is encoded at, in kb/s, where the codec takes one.
const AUDIO_KBPS: u32 = 192;
/// The bit rate VP8 and Theora video is encoded at, in kb/s: without one they look poor. x264
/// and x265 pick their own from a constant quality.
const VIDEO_KBPS: u32 = 2500;

impl Preset {
    /// Every preset, as the dialog lists them.
    pub const ALL: [Self; 8] = [
        Self::Mp4H264,
        Self::Mp4H265,
        Self::WebM,
        Self::Ogg,
        Self::Mp3,
        Self::Aac,
        Self::Vorbis,
        Self::Flac,
    ];

    /// Its name in the dialog.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mp4H264 => tr!("Video – H.264 + AAC (MP4)"),
            Self::Mp4H265 => tr!("Video – H.265 + AAC (MP4)"),
            Self::WebM => tr!("Video – VP8 + Vorbis (WebM)"),
            Self::Ogg => tr!("Video – Theora + Vorbis (OGG)"),
            Self::Mp3 => tr!("Audio – MP3"),
            Self::Aac => tr!("Audio – AAC (M4A)"),
            Self::Vorbis => tr!("Audio – Vorbis (OGG)"),
            Self::Flac => tr!("Audio – FLAC"),
        }
    }

    /// The extension its files get.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4H264 | Self::Mp4H265 => "mp4",
            Self::WebM => "webm",
            Self::Ogg => "ogv",
            Self::Mp3 => "mp3",
            Self::Aac => "m4a",
            Self::Vorbis => "ogg",
            Self::Flac => "flac",
        }
    }

    /// Whether it keeps only the sound.
    pub fn is_audio_only(self) -> bool {
        matches!(self, Self::Mp3 | Self::Aac | Self::Vorbis | Self::Flac)
    }

    /// The media options converting to it into `output`: the `sout` chain, then which streams
    /// go through. Subtitles never do: few of these containers hold them.
    pub fn media_options(self, output: &Path) -> Vec<String> {
        let mut options =
            vec![format!(":sout={}", self.stream_output(output)), ":no-sout-spu".into()];
        if self.is_audio_only() {
            options.push(":no-sout-video".into());
        }
        options
    }

    /// The `sout` chain: encode, then write the container to `output`.
    fn stream_output(self, output: &Path) -> String {
        let destination = quoted(&output.to_string_lossy());
        format!(
            "#transcode{{{}}}:std{{access=file,mux={},dst={destination}}}",
            self.codecs(),
            self.mux()
        )
    }

    /// The `transcode` options.
    fn codecs(self) -> String {
        let aac = format!("acodec=mp4a,ab={AUDIO_KBPS}");
        let vorbis = format!("acodec=vorb,ab={AUDIO_KBPS}");
        match self {
            Self::Mp4H264 => format!("vcodec=h264,{aac}"),
            Self::Mp4H265 => format!("vcodec=hevc,{aac}"),
            Self::WebM => format!("vcodec=VP80,vb={VIDEO_KBPS},{vorbis}"),
            Self::Ogg => format!("vcodec=theo,vb={VIDEO_KBPS},{vorbis}"),
            // LAME takes two channels at most.
            Self::Mp3 => format!("vcodec=none,acodec=mp3,ab={AUDIO_KBPS},channels=2"),
            Self::Aac => format!("vcodec=none,{aac}"),
            Self::Vorbis => format!("vcodec=none,{vorbis}"),
            Self::Flac => "vcodec=none,acodec=flac".to_owned(),
        }
    }

    /// The muxer writing the container.
    fn mux(self) -> &'static str {
        match self {
            Self::Mp4H264 | Self::Mp4H265 | Self::Aac => "mp4",
            Self::WebM => "webm",
            Self::Ogg | Self::Vorbis => "ogg",
            Self::Mp3 | Self::Flac => "raw",
        }
    }
}

/// `text` as a quoted `sout` value: in single quotes, with backslashes and single quotes
/// escaped, so a path's commas, braces and quotes stay in it.
fn quoted(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp4_encodes_h264_and_aac_into_the_file() {
        let options = Preset::Mp4H264.media_options(Path::new("/out/a.mp4"));
        assert_eq!(
            options[0],
            ":sout=#transcode{vcodec=h264,acodec=mp4a,ab=192}:std{access=file,mux=mp4,dst='/out/a.mp4'}"
        );
        assert_eq!(options[1..], [":no-sout-spu"]);
    }

    #[test]
    fn audio_presets_drop_the_video() {
        for preset in Preset::ALL.into_iter().filter(|preset| preset.is_audio_only()) {
            let options = preset.media_options(Path::new("/a.x"));
            assert!(options.contains(&":no-sout-video".to_owned()), "{preset:?}");
            assert!(options[0].contains("vcodec=none"), "{preset:?}");
        }
    }

    #[test]
    fn quotes_and_backslashes_in_paths_are_escaped() {
        assert_eq!(quoted(r"C:\it's {here}, ok"), r"'C:\\it\'s {here}, ok'");
    }

    #[test]
    fn every_preset_has_a_name_and_an_extension() {
        for preset in Preset::ALL {
            assert!(!preset.name().is_empty() && !preset.extension().is_empty());
        }
    }
}
