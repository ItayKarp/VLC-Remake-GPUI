//! Command-line options of the spike.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};

const USAGE: &str = "usage: video_spike [--path cpu|surface] [--seconds N] [--max-texture N] \
                     [--libvlc DIR] [--vlc-arg ARG]... FILE";

/// The libVLC runtime the spike uses unless `--libvlc` says otherwise.
const DEFAULT_LIBVLC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/libvlc/macos-arm64");
/// How long a measurement runs.
const DEFAULT_SECONDS: u64 = 60;
/// The largest texture side every target GPU accepts (S1.1 edge case: downscale above it).
const DEFAULT_MAX_TEXTURE: u32 = 8192;

/// How decoded frames reach the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentPath {
    /// (a) BGRA in CPU memory, uploaded as a GPUI `RenderImage` each frame.
    Cpu,
    /// (b) NV12 `CVPixelBuffer` drawn by `gpui::surface()` (macOS only).
    Surface,
}

impl PresentPath {
    /// The name used on the command line and in reports.
    pub fn name(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Surface => "surface",
        }
    }
}

/// Everything the command line can set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// The file to play.
    pub file: PathBuf,
    /// The presentation path under test.
    pub path: PresentPath,
    /// Seconds to play before printing the summary and quitting.
    pub seconds: u64,
    /// Video wider or taller than this is scaled down by libVLC.
    pub max_texture: u32,
    /// The unpacked VLC folder holding `lib/` (macOS) and `plugins/`.
    pub libvlc: Option<PathBuf>,
    /// Extra libVLC arguments, such as `--avcodec-hw=none`.
    pub vlc_arguments: Vec<String>,
}

impl Options {
    /// Parses `arguments` (without the program name).
    pub fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Self> {
        let mut options = Self {
            file: PathBuf::new(),
            path: PresentPath::Cpu,
            seconds: DEFAULT_SECONDS,
            max_texture: DEFAULT_MAX_TEXTURE,
            libvlc: cfg!(target_os = "macos").then(|| DEFAULT_LIBVLC.into()),
            vlc_arguments: Vec::new(),
        };
        let mut arguments = arguments.into_iter();
        while let Some(argument) = arguments.next() {
            let mut value =
                || arguments.next().with_context(|| format!("{argument} needs a value"));
            match argument.as_str() {
                "--path" => options.path = parse_path(&value()?)?,
                "--seconds" => options.seconds = value()?.parse().context("--seconds")?,
                "--max-texture" => {
                    options.max_texture = value()?.parse().context("--max-texture")?;
                }
                "--libvlc" => options.libvlc = Some(value()?.into()),
                "--vlc-arg" => options.vlc_arguments.push(value()?),
                flag if flag.starts_with("--") => bail!("unknown option {flag}\n{USAGE}"),
                _ => options.file = argument.into(),
            }
        }
        if options.file.as_os_str().is_empty() {
            bail!("no file given\n{USAGE}");
        }
        Ok(options)
    }
}

fn parse_path(name: &str) -> Result<PresentPath> {
    match name {
        "cpu" => Ok(PresentPath::Cpu),
        "surface" if cfg!(target_os = "macos") => Ok(PresentPath::Surface),
        "surface" => bail!("the surface path exists only on macOS"),
        other => bail!("unknown path {other}; expected cpu or surface"),
    }
}

#[cfg(test)]
mod tests {
    use super::{Options, PresentPath};

    fn parse(line: &str) -> anyhow::Result<Options> {
        Options::parse(line.split_whitespace().map(String::from))
    }

    #[test]
    fn a_file_alone_uses_the_defaults() {
        let options = parse("movie.mkv").unwrap();
        assert_eq!(options.file.to_str(), Some("movie.mkv"));
        assert_eq!(options.path, PresentPath::Cpu);
        assert_eq!((options.seconds, options.max_texture), (60, 8192));
    }

    #[test]
    fn flags_are_read_and_libvlc_arguments_collected() {
        let options =
            parse("--seconds 5 --vlc-arg --avcodec-hw=none --vlc-arg -q --max-texture 4096 a.mp4")
                .unwrap();
        assert_eq!(options.seconds, 5);
        assert_eq!(options.max_texture, 4096);
        assert_eq!(options.vlc_arguments, ["--avcodec-hw=none", "-q"]);
    }

    #[test]
    fn mistakes_are_reported() {
        assert!(parse("").is_err());
        assert!(parse("--seconds").is_err());
        assert!(parse("--path gpu a.mp4").is_err());
        assert!(parse("--frobnicate a.mp4").is_err());
    }
}
