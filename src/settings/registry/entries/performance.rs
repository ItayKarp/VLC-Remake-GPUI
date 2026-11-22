//! Performance ("Performance Settings", `performance.png`, S5.10.2): decoding, keeping up, VLC's
//! priority and, under Advanced Options, the Developer settings. They are libVLC startup
//! options, so changes apply at the next launch.

use gpui::{App, PromptLevel, Window};

#[cfg(not(any(windows, target_os = "macos")))]
use super::super::def::Condition;
use super::super::def::{ApplyMode, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, choice, number, toggle};
use crate::app::hardware_fallback;
#[cfg(not(target_os = "macos"))]
use crate::app::real_time;
use crate::settings::registry::Category;
use crate::settings::{
    self, ClockSynchronisation, Decoder, Encoder, HardwareDecoding, PostProcessing, SkipLoopFilter,
};

const DECODING: (Category, &str) = (Category::Performance, "Decoding");
const SMOOTH: (Category, &str) = (Category::Performance, "Smooth Playback");
const DEVELOPER: (Category, &str) = (Category::Performance, "Developer");

/// The key of "Give VLC real-time priority", which "Process priority" depends on.
#[cfg(not(any(windows, target_os = "macos")))]
const REAL_TIME_KEY: &str = "performance.real_time_priority";

impl SettingChoice for HardwareDecoding {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Off];
    const CHOICES: &'static [Choice] =
        &[Choice::new("automatic", "Automatic"), Choice::new("off", "Off")];
}

impl SettingChoice for PostProcessing {
    const VALUES: &'static [Self] = &[Self::Off, Self::Low, Self::Medium, Self::High];
    const CHOICES: &'static [Choice] = &[
        Choice::new("off", "Off"),
        Choice::new("low", "Low"),
        Choice::new("medium", "Medium"),
        Choice::new("high", "High"),
    ];
}

impl SettingChoice for SkipLoopFilter {
    const VALUES: &'static [Self] =
        &[Self::Never, Self::NonReference, Self::Bidirectional, Self::NonKey, Self::Always];
    const CHOICES: &'static [Choice] = &[
        Choice::new("never", "Never"),
        Choice::new("non_reference", "Non-reference frames"),
        Choice::new("bidirectional", "Bidirectional frames"),
        Choice::new("non_key", "Non-key frames"),
        Choice::new("always", "Always"),
    ];
}

impl SettingChoice for Encoder {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::X264, Self::X265, Self::Ffmpeg];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", "Automatic"),
        Choice::new("x264", "x264 (H.264)"),
        Choice::new("x265", "x265 (HEVC)"),
        Choice::new("ffmpeg", "FFmpeg"),
    ];
}

impl SettingChoice for Decoder {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Ffmpeg];
    const CHOICES: &'static [Choice] =
        &[Choice::new("automatic", "Automatic"), Choice::new("ffmpeg", "FFmpeg")];
}

impl SettingChoice for ClockSynchronisation {
    const VALUES: &'static [Self] = &[Self::Default, Self::On, Self::Off];
    const CHOICES: &'static [Choice] =
        &[Choice::new("default", "Default"), Choice::new("on", "On"), Choice::new("off", "Off")];
}

/// Says when more decoding threads were asked for than the machine has cores.
fn threads_hint(cx: &App) -> Option<String> {
    let threads = settings::get(cx).prefs.performance.decoding_threads;
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    (threads as usize > cores)
        .then(|| format!("This computer has {cores} cores; more threads rarely help."))
}

/// Says why real-time priority, while it's on, isn't used: the system refuses it.
#[cfg(not(target_os = "macos"))]
fn real_time_hint(cx: &App) -> Option<String> {
    let wanted = settings::get(cx).prefs.performance.real_time_priority;
    wanted.then(real_time::refusal).flatten().map(str::to_owned)
}

/// "Rescan plugins": the next launch looks for libVLC's plugins again; offers to restart now.
fn rescan_plugins(window: &mut Window, cx: &mut App) {
    settings::update(cx, |settings| settings.local.rescan_plugins = true);
    let answer = window.prompt(
        PromptLevel::Info,
        "VLC looks for its plugins again when it restarts",
        Some("Do this after adding or removing libVLC plugins by hand."),
        &["Restart Now", "Later"],
        cx,
    );
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            cx.update(settings::restart);
        }
    })
    .detach();
}

/// The Performance page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "performance.hardware_decoding",
        DECODING,
        "Hardware-accelerated decoding",
        Control::Dropdown(HardwareDecoding::CHOICES),
        choice!(prefs.performance.hardware_decoding),
    )
    .info(
        "Lets the graphics card decode video, which saves power and CPU. Turn it off if video \
         shows green or garbled frames.",
    )
    .danger_hint(hardware_fallback::hint)
    .applies(ApplyMode::Restart)
    .keywords(&["gpu", "hardware decoding", "videotoolbox", "dxva", "vaapi", "acceleration"]),
    SettingDef::new(
        "performance.post_processing",
        DECODING,
        "Video post-processing",
        Control::Dropdown(PostProcessing::CHOICES),
        choice!(prefs.performance.post_processing),
    )
    .info("Smooths blocky, low-bitrate video. Higher levels cost more CPU.")
    .applies(ApplyMode::Restart)
    .keywords(&["deblocking", "quality", "postproc", "artifacts"]),
    SettingDef::new(
        "performance.skip_loop_filter",
        DECODING,
        "Skip the H.264 loop filter",
        Control::Dropdown(SkipLoopFilter::CHOICES),
        choice!(prefs.performance.skip_loop_filter),
    )
    .info(
        "Skipping the filter that smooths block edges in H.264 video speeds decoding up on a \
         slow computer, at some cost in picture quality.",
    )
    .applies(ApplyMode::Restart)
    .keywords(&["deblocking", "avc", "slow computer"]),
    SettingDef::new(
        "performance.decoding_threads",
        DECODING,
        "Decoding threads",
        Control::Number { unit: "" },
        number!(u32: prefs.performance.decoding_threads),
    )
    .info("How many threads decode video. 0 lets VLC choose for this computer.")
    .range(0., 64., 1.)
    .hint(threads_hint)
    .applies(ApplyMode::Restart)
    .keywords(&["cpu", "cores", "multithreading"]),
    SettingDef::new(
        "performance.drop_late_frames",
        SMOOTH,
        "Drop frames when the system can't keep up",
        Control::Toggle,
        toggle!(prefs.performance.drop_late_frames),
    )
    .info("Skips pictures that are too late, so sound and picture stay together.")
    .applies(ApplyMode::Restart)
    .keywords(&["frame drop", "late frames", "sync", "stutter"]),
    // macOS's libVLC has no real-time priority.
    #[cfg(not(target_os = "macos"))]
    SettingDef::new(
        "performance.real_time_priority",
        SMOOTH,
        "Give VLC real-time priority",
        Control::Toggle,
        toggle!(prefs.performance.real_time_priority),
    )
    .info(
        "Runs VLC ahead of other programs. It can make the computer sluggish while playing, and \
         some systems allow it only to administrators; the row says when it isn't allowed.",
    )
    .danger_hint(real_time_hint)
    .applies(ApplyMode::Restart)
    .keywords(&["priority", "realtime", "scheduling"]),
    // Windows has no priority offset: real-time priority there is the high priority class.
    #[cfg(not(any(windows, target_os = "macos")))]
    SettingDef::new(
        "performance.priority_offset",
        SMOOTH,
        "Process priority",
        Control::Number { unit: "" },
        number!(i32: prefs.performance.priority_offset),
    )
    .info(
        "Raises (above 0) or lowers (below 0) the priority of VLC's real-time threads against \
         other programs.",
    )
    .range(-10., 10., 1.)
    .depends_on(REAL_TIME_KEY, Condition::IsOn)
    .applies(ApplyMode::Restart)
    .keywords(&["priority", "nice", "rt offset"]),
    SettingDef::new(
        "rescan_plugins",
        SMOOTH,
        "Rescan plugins",
        Control::Button { label: "Rescan", action: rescan_plugins },
        Binding::ACTION,
    )
    .info("Has VLC look for its plugins again the next time it starts, instead of its cache.")
    .keywords(&["plugins cache", "modules", "reset"]),
    SettingDef::new(
        "performance.encoder",
        DEVELOPER,
        "Encoder",
        Control::Dropdown(Encoder::CHOICES),
        choice!(prefs.performance.encoder),
    )
    .info("The encoder recording and streaming try first.")
    .applies(ApplyMode::Restart)
    .keywords(&["codec", "x264", "x265", "ffmpeg", "encoding"])
    .advanced(),
    SettingDef::new(
        "performance.decoder",
        DEVELOPER,
        "Decoder",
        Control::Dropdown(Decoder::CHOICES),
        choice!(prefs.performance.decoder),
    )
    .info("The decoder playback tries first.")
    .applies(ApplyMode::Restart)
    .keywords(&["codec", "ffmpeg", "avcodec", "decoding"])
    .advanced(),
    SettingDef::new(
        "performance.clock_synchronisation",
        DEVELOPER,
        "Clock synchronisation",
        Control::Dropdown(ClockSynchronisation::CHOICES),
        choice!(prefs.performance.clock_synchronisation),
    )
    .info(
        "Whether playback follows the pace of a live source, such as a TV stream, instead of \
         this computer's clock.",
    )
    .applies(ApplyMode::Restart)
    .keywords(&["clock", "sync", "live", "drift"])
    .advanced(),
    SettingDef::new(
        "performance.clock_jitter_ms",
        DEVELOPER,
        "Clock jitter",
        Control::Number { unit: "ms" },
        number!(u32: prefs.performance.clock_jitter_ms),
    )
    .info("How far a source's clock may wander before VLC resynchronises with it.")
    .range(0., 60_000., 10.)
    .applies(ApplyMode::Restart)
    .keywords(&["clock", "jitter", "sync"])
    .advanced(),
    SettingDef::new(
        "performance.clock_average",
        DEVELOPER,
        "Clock reference averaging",
        Control::Number { unit: "" },
        number!(u32: prefs.performance.clock_average),
    )
    .info("How many of a source's clock readings VLC averages. More is steadier but slower to follow.")
    .range(1., 1000., 1.)
    .applies(ApplyMode::Restart)
    .keywords(&["clock", "cr average", "sync"])
    .advanced(),
];
