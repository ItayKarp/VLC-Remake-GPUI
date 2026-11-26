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

const DECODING: (Category, &str) = (Category::Performance, tr_noop!("Decoding"));
const SMOOTH: (Category, &str) = (Category::Performance, tr_noop!("Smooth Playback"));
const DEVELOPER: (Category, &str) = (Category::Performance, tr_noop!("Developer"));

/// The key of "Give VLC real-time priority", which "Process priority" depends on.
#[cfg(not(any(windows, target_os = "macos")))]
const REAL_TIME_KEY: &str = "performance.real_time_priority";

impl SettingChoice for HardwareDecoding {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Off];
    const CHOICES: &'static [Choice] =
        &[Choice::new("automatic", tr_noop!("Automatic")), Choice::new("off", tr_noop!("Off"))];
}

impl SettingChoice for PostProcessing {
    const VALUES: &'static [Self] = &[Self::Off, Self::Low, Self::Medium, Self::High];
    const CHOICES: &'static [Choice] = &[
        Choice::new("off", tr_noop!("Off")),
        Choice::new("low", tr_noop!("Low")),
        Choice::new("medium", tr_noop!("Medium")),
        Choice::new("high", tr_noop!("High")),
    ];
}

impl SettingChoice for SkipLoopFilter {
    const VALUES: &'static [Self] =
        &[Self::Never, Self::NonReference, Self::Bidirectional, Self::NonKey, Self::Always];
    const CHOICES: &'static [Choice] = &[
        Choice::new("never", tr_noop!("Never")),
        Choice::new("non_reference", tr_noop!("Non-reference frames")),
        Choice::new("bidirectional", tr_noop!("Bidirectional frames")),
        Choice::new("non_key", tr_noop!("Non-key frames")),
        Choice::new("always", tr_noop!("Always")),
    ];
}

impl SettingChoice for Encoder {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::X264, Self::X265, Self::Ffmpeg];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", tr_noop!("Automatic")),
        Choice::new("x264", tr_noop!("x264 (H.264)")),
        Choice::new("x265", tr_noop!("x265 (HEVC)")),
        Choice::new("ffmpeg", tr_noop!("FFmpeg")),
    ];
}

impl SettingChoice for Decoder {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::Ffmpeg];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", tr_noop!("Automatic")),
        Choice::new("ffmpeg", tr_noop!("FFmpeg")),
    ];
}

impl SettingChoice for ClockSynchronisation {
    const VALUES: &'static [Self] = &[Self::Default, Self::On, Self::Off];
    const CHOICES: &'static [Choice] = &[
        Choice::new("default", tr_noop!("Default")),
        Choice::new("on", tr_noop!("On")),
        Choice::new("off", tr_noop!("Off")),
    ];
}

/// Says when more decoding threads were asked for than the machine has cores.
fn threads_hint(cx: &App) -> Option<String> {
    let threads = settings::get(cx).prefs.performance.decoding_threads;
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    (threads as usize > cores).then(|| {
        trn!(
            "This computer has {count} core; more threads rarely help.",
            "This computer has {count} cores; more threads rarely help.",
            cores
        )
    })
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
        tr!("VLC looks for its plugins again when it restarts"),
        Some(tr!("Do this after adding or removing libVLC plugins by hand.")),
        &[tr!("Restart Now"), tr!("Later")],
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
        tr_noop!("Hardware-accelerated decoding"),
        Control::Dropdown(HardwareDecoding::CHOICES),
        choice!(prefs.performance.hardware_decoding),
    )
    .info(tr_noop!(
        "Lets the graphics card decode video, which saves power and CPU. Turn it off if video \
         shows green or garbled frames."
    ))
    .danger_hint(hardware_fallback::hint)
    .applies(ApplyMode::Restart)
    .keywords(&["gpu", "hardware decoding", "videotoolbox", "dxva", "vaapi", "acceleration"]),
    SettingDef::new(
        "performance.post_processing",
        DECODING,
        tr_noop!("Video post-processing"),
        Control::Dropdown(PostProcessing::CHOICES),
        choice!(prefs.performance.post_processing),
    )
    .info(tr_noop!("Smooths blocky, low-bitrate video. Higher levels cost more CPU."))
    .applies(ApplyMode::Restart)
    .keywords(&["deblocking", "quality", "postproc", "artifacts"]),
    SettingDef::new(
        "performance.skip_loop_filter",
        DECODING,
        tr_noop!("Skip the H.264 loop filter"),
        Control::Dropdown(SkipLoopFilter::CHOICES),
        choice!(prefs.performance.skip_loop_filter),
    )
    .info(tr_noop!(
        "Skipping the filter that smooths block edges in H.264 video speeds decoding up on a slow \
         computer, at some cost in picture quality."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["deblocking", "avc", "slow computer"]),
    SettingDef::new(
        "performance.decoding_threads",
        DECODING,
        tr_noop!("Decoding threads"),
        Control::Number { unit: "" },
        number!(u32: prefs.performance.decoding_threads),
    )
    .info(tr_noop!("How many threads decode video. 0 lets VLC choose for this computer."))
    .range(0., 64., 1.)
    .hint(threads_hint)
    .applies(ApplyMode::Restart)
    .keywords(&["cpu", "cores", "multithreading"]),
    SettingDef::new(
        "performance.drop_late_frames",
        SMOOTH,
        tr_noop!("Drop frames when the system can't keep up"),
        Control::Toggle,
        toggle!(prefs.performance.drop_late_frames),
    )
    .info(tr_noop!("Skips pictures that are too late, so sound and picture stay together."))
    .applies(ApplyMode::Restart)
    .keywords(&["frame drop", "late frames", "sync", "stutter"]),
    // macOS's libVLC has no real-time priority.
    #[cfg(not(target_os = "macos"))]
    SettingDef::new(
        "performance.real_time_priority",
        SMOOTH,
        tr_noop!("Give VLC real-time priority"),
        Control::Toggle,
        toggle!(prefs.performance.real_time_priority),
    )
    .info(tr_noop!(
        "Runs VLC ahead of other programs. It can make the computer sluggish while playing, and \
         some systems allow it only to administrators; the row says when it isn't allowed."
    ))
    .danger_hint(real_time_hint)
    .applies(ApplyMode::Restart)
    .keywords(&["priority", "realtime", "scheduling"]),
    // Windows has no priority offset: real-time priority there is the high priority class.
    #[cfg(not(any(windows, target_os = "macos")))]
    SettingDef::new(
        "performance.priority_offset",
        SMOOTH,
        tr_noop!("Process priority"),
        Control::Number { unit: "" },
        number!(i32: prefs.performance.priority_offset),
    )
    .info(tr_noop!(
        "Raises (above 0) or lowers (below 0) the priority of VLC's real-time threads against \
         other programs."
    ))
    .range(-10., 10., 1.)
    .depends_on(REAL_TIME_KEY, Condition::IsOn)
    .applies(ApplyMode::Restart)
    .keywords(&["priority", "nice", "rt offset"]),
    SettingDef::new(
        "rescan_plugins",
        SMOOTH,
        tr_noop!("Rescan plugins"),
        Control::Button { label: tr_noop!("Rescan"), action: rescan_plugins },
        Binding::ACTION,
    )
    .info(tr_noop!(
        "Has VLC look for its plugins again the next time it starts, instead of its cache."
    ))
    .keywords(&["plugins cache", "modules", "reset"]),
    SettingDef::new(
        "performance.encoder",
        DEVELOPER,
        tr_noop!("Encoder"),
        Control::Dropdown(Encoder::CHOICES),
        choice!(prefs.performance.encoder),
    )
    .info(tr_noop!("The encoder recording and streaming try first."))
    .applies(ApplyMode::Restart)
    .keywords(&["codec", "x264", "x265", "ffmpeg", "encoding"])
    .advanced(),
    SettingDef::new(
        "performance.decoder",
        DEVELOPER,
        tr_noop!("Decoder"),
        Control::Dropdown(Decoder::CHOICES),
        choice!(prefs.performance.decoder),
    )
    .info(tr_noop!("The decoder playback tries first."))
    .applies(ApplyMode::Restart)
    .keywords(&["codec", "ffmpeg", "avcodec", "decoding"])
    .advanced(),
    SettingDef::new(
        "performance.clock_synchronisation",
        DEVELOPER,
        tr_noop!("Clock synchronisation"),
        Control::Dropdown(ClockSynchronisation::CHOICES),
        choice!(prefs.performance.clock_synchronisation),
    )
    .info(tr_noop!(
        "Whether playback follows the pace of a live source, such as a TV stream, instead of this \
         computer's clock."
    ))
    .applies(ApplyMode::Restart)
    .keywords(&["clock", "sync", "live", "drift"])
    .advanced(),
    SettingDef::new(
        "performance.clock_jitter_ms",
        DEVELOPER,
        tr_noop!("Clock jitter"),
        Control::Number { unit: tr_noop!("ms") },
        number!(u32: prefs.performance.clock_jitter_ms),
    )
    .info(tr_noop!("How far a source's clock may wander before VLC resynchronises with it."))
    .range(0., 60_000., 10.)
    .applies(ApplyMode::Restart)
    .keywords(&["clock", "jitter", "sync"])
    .advanced(),
    SettingDef::new(
        "performance.clock_average",
        DEVELOPER,
        tr_noop!("Clock reference averaging"),
        Control::Number { unit: "" },
        number!(u32: prefs.performance.clock_average),
    )
    .info(tr_noop!(
        "How many of a source's clock readings VLC averages. More is steadier but slower to follow."
    ))
    .range(1., 1000., 1.)
    .applies(ApplyMode::Restart)
    .keywords(&["clock", "cr average", "sync"])
    .advanced(),
];
