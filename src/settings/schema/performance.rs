//! Performance settings (Settings → Performance, S5.10.2): how video is decoded, how playback
//! keeps up, VLC's priority, and the clock settings developers tune.

use serde::{Deserialize, Serialize};

use super::super::lenient::lenient;

/// The clock jitter libVLC tolerates by default, in milliseconds.
pub const DEFAULT_CLOCK_JITTER_MS: u32 = 5000;
/// How many clock references libVLC averages by default.
pub const DEFAULT_CLOCK_AVERAGE: u32 = 40;

/// Performance preferences (S5.10.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PerformanceSettings {
    /// "Hardware-accelerated decoding".
    #[serde(deserialize_with = "lenient")]
    pub hardware_decoding: HardwareDecoding,
    /// "Video post-processing": how hard the decoder cleans up blocky video.
    #[serde(deserialize_with = "lenient")]
    pub post_processing: PostProcessing,
    /// "Skip the H.264 loop filter": which frames skip the deblocking filter.
    #[serde(deserialize_with = "lenient")]
    pub skip_loop_filter: SkipLoopFilter,
    /// "Decoding threads"; `0` lets the decoder pick.
    #[serde(deserialize_with = "lenient")]
    pub decoding_threads: u32,
    /// "Drop frames when the system can't keep up".
    #[serde(deserialize_with = "lenient")]
    pub drop_late_frames: bool,
    /// "Give VLC real-time priority".
    #[serde(deserialize_with = "lenient")]
    pub real_time_priority: bool,
    /// "Process priority": added to the priority of libVLC's threads; higher runs sooner.
    #[serde(deserialize_with = "lenient")]
    pub priority_offset: i32,
    /// "Encoder" (Developer).
    #[serde(deserialize_with = "lenient")]
    pub encoder: Encoder,
    /// "Decoder" (Developer).
    #[serde(deserialize_with = "lenient")]
    pub decoder: Decoder,
    /// "Clock synchronisation" (Developer).
    #[serde(deserialize_with = "lenient")]
    pub clock_synchronisation: ClockSynchronisation,
    /// "Clock jitter" (Developer), in milliseconds.
    #[serde(deserialize_with = "lenient")]
    pub clock_jitter_ms: u32,
    /// "Clock reference averaging" (Developer): how many references the clock averages.
    #[serde(deserialize_with = "lenient")]
    pub clock_average: u32,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            hardware_decoding: HardwareDecoding::default(),
            post_processing: PostProcessing::default(),
            skip_loop_filter: SkipLoopFilter::default(),
            decoding_threads: 0,
            drop_late_frames: true,
            real_time_priority: false,
            priority_offset: 0,
            encoder: Encoder::default(),
            decoder: Decoder::default(),
            clock_synchronisation: ClockSynchronisation::default(),
            clock_jitter_ms: DEFAULT_CLOCK_JITTER_MS,
            clock_average: DEFAULT_CLOCK_AVERAGE,
        }
    }
}

/// "Hardware-accelerated decoding".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareDecoding {
    /// The GPU decodes where it can, falling back to the CPU.
    #[default]
    Automatic,
    /// The CPU always decodes.
    Off,
}

/// "Video post-processing": the decoder's post-processing quality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostProcessing {
    /// None.
    #[default]
    Off,
    /// Light deblocking.
    Low,
    /// Deblocking and deringing.
    Medium,
    /// Everything the decoder can do; costs the most CPU.
    High,
}

/// "Skip the H.264 loop filter": skipping it speeds decoding up at some cost in quality.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipLoopFilter {
    /// Always filter.
    #[default]
    Never,
    /// Skip it on frames no other frame refers to.
    NonReference,
    /// Skip it on bidirectional frames.
    Bidirectional,
    /// Skip it on every frame but keyframes.
    NonKey,
    /// Skip it everywhere.
    Always,
}

/// "Encoder": the encoder recording and streaming prefer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoder {
    /// libVLC picks.
    #[default]
    Automatic,
    /// x264 (H.264).
    X264,
    /// x265 (HEVC).
    X265,
    /// FFmpeg's encoders.
    Ffmpeg,
}

/// "Decoder": the decoder playback prefers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decoder {
    /// libVLC picks.
    #[default]
    Automatic,
    /// FFmpeg's decoders first.
    Ffmpeg,
}

/// "Clock synchronisation": whether the clock follows a live source's pace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClockSynchronisation {
    /// libVLC decides per source.
    #[default]
    Default,
    /// Always.
    On,
    /// Never.
    Off,
}
