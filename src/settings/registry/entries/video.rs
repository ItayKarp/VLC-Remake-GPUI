//! Video ("Video Defaults", S5.8.1): the output, the picture and deinterlacing every video shows
//! with; the framing, flip and rotation are in `video_framing.rs`. The in-player panels (Stage 7)
//! change them for the session only (S5.8.4, D24).
//!
//! "Play video as desktop wallpaper" is only offered on Windows (S8.16.3): a window behind the
//! desktop icons only exists there.

use super::super::def::{ApplyMode, Condition, Control, Marks, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, choice, number, toggle};
use crate::engine::media_options::{Deinterlace, DeinterlaceMethod};
use crate::settings::registry::Category;
use crate::settings::{self, PictureSettings};

const OUTPUT: (Category, &str) = (Category::Video, "Output");
const PICTURE: (Category, &str) = (Category::Video, "Picture");
const DEINTERLACING: (Category, &str) = (Category::Video, "Deinterlacing");
/// What the picture rows change, and how they relate to the player's panel (D24).
const PICTURE_INFO: &str = "Seen at once, and kept for every video. The player's settings panel \
                            can change it for the rest of the session.";
/// What an enhancement's slider says at 0.
const OFF: &str = "Off";

impl SettingChoice for Deinterlace {
    const VALUES: &'static [Self] = &[Self::Automatic, Self::On, Self::Off];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", "Automatic"),
        Choice::new("on", "On"),
        Choice::new("off", "Off"),
    ];
}

impl SettingChoice for DeinterlaceMethod {
    const VALUES: &'static [Self] = &[
        Self::Automatic,
        Self::Discard,
        Self::Blend,
        Self::Mean,
        Self::Bob,
        Self::Linear,
        Self::X,
        Self::Yadif,
        Self::Yadif2x,
        Self::Phosphor,
        Self::Ivtc,
    ];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", "Automatic"),
        Choice::new("discard", "Discard"),
        Choice::new("blend", "Blend"),
        Choice::new("mean", "Mean"),
        Choice::new("bob", "Bob"),
        Choice::new("linear", "Linear"),
        Choice::new("x", "X"),
        Choice::new("yadif", "Yadif"),
        Choice::new("yadif2x", "Yadif (2x)"),
        Choice::new("phosphor", "Phosphor"),
        Choice::new("ivtc", "Film NTSC (IVTC)"),
    ];
}

/// A picture value as its slider says it: "1.25".
fn plain(value: f64) -> String {
    super::super::rules::trim(value)
}

/// A hue as its slider says it: "-30°".
fn degrees(value: f64) -> String {
    format!("{value:.0}°")
}

/// An enhancement's strength as its slider says it: "Off" at 0, else "0.5".
fn strength(value: f64) -> String {
    if value > 0. { plain(value) } else { OFF.to_owned() }
}

/// A banding removal radius as its slider says it: "Off" at 0, else "16 px".
fn radius(value: f64) -> String {
    if value > 0. { format!("{value:.0} px") } else { OFF.to_owned() }
}

/// Puts every picture value back, here and in the player's panel (S7.3.4), leaving "Adjust the
/// picture" as it is.
fn reset_picture(_: &mut gpui::Window, cx: &mut gpui::App) {
    settings::update(cx, |settings| {
        let picture = &mut settings.prefs.video.picture;
        *picture = PictureSettings { enabled: picture.enabled, ..PictureSettings::default() };
    });
    crate::effects::reset(cx, crate::effects::VIDEO_ESSENTIAL.controls.iter().copied());
}

/// Says when the player's panel has changed the picture for this session, so this page's values
/// aren't what plays (D24).
fn picture_overridden(cx: &gpui::App) -> Option<String> {
    crate::effects::any_changed(cx, &crate::effects::VIDEO_ESSENTIAL).then(|| {
        "The player's settings panel changed these for this session; its Save as Default \
         writes them here."
            .to_owned()
    })
}

/// A picture row: `slider` over `range` (minimum, maximum, step).
const fn picture_row(
    key: &'static str,
    label: &'static str,
    slider: (Control, Binding),
    range: (f64, f64, f64),
) -> SettingDef {
    SettingDef::new(key, PICTURE, label, slider.0, slider.1)
        .info(PICTURE_INFO)
        .range(range.0, range.1, range.2)
        .depends_on("video.picture.enabled", Condition::IsOn)
        .keywords(&["color", "adjust", "image", "filter"])
}

/// The Video page's rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "video.enabled",
        OUTPUT,
        "Enable video",
        Control::Toggle,
        toggle!(prefs.video.enabled),
    )
    .info(
        "Off, media plays its sound only: video isn't decoded, which saves power, and the video \
         area says why it is empty.",
    )
    .applies(ApplyMode::NextMedia)
    .keywords(&["audio only", "black screen", "decode", "battery"]),
    SettingDef::new(
        "video.start_fullscreen",
        OUTPUT,
        "Start videos in fullscreen",
        Control::Toggle,
        toggle!(prefs.video.start_fullscreen),
    )
    .info("Each video goes fullscreen as it starts. Escape leaves fullscreen.")
    .keywords(&["full screen", "maximize"]),
    SettingDef::new(
        "video.keep_display_awake",
        OUTPUT,
        "Stop the screensaver during playback",
        Control::Toggle,
        toggle!(prefs.video.keep_display_awake),
    )
    .info(
        "Keeps the display from sleeping or starting the screensaver while video plays in a \
         window that shows.",
    )
    .keywords(&["sleep", "display", "power", "idle", "inhibit"]),
    #[cfg(target_os = "windows")]
    SettingDef::new(
        "video.desktop_wallpaper",
        OUTPUT,
        "Play video as desktop wallpaper",
        Control::Toggle,
        toggle!(prefs.video.desktop_wallpaper),
    )
    .info(
        "Plays video behind the desktop icons on the main display, instead of in a window. The \
         main window keeps the controls.",
    )
    .keywords(&["wallpaper", "background", "desktop", "live wallpaper"]),
    SettingDef::new(
        "video.picture.enabled",
        PICTURE,
        "Adjust the picture",
        Control::Toggle,
        toggle!(prefs.video.picture.enabled),
    )
    .info(PICTURE_INFO)
    .hint(picture_overridden)
    .keywords(&["color", "adjust", "image", "filter", "essential"]),
    picture_row(
        "video.picture.hue",
        "Hue",
        (
            Control::Slider { marks: Marks::None, format: degrees },
            number!(f32: prefs.video.picture.hue),
        ),
        (-180., 180., 1.),
    ),
    picture_row(
        "video.picture.brightness",
        "Brightness",
        (
            Control::Slider { marks: Marks::None, format: plain },
            number!(f32: prefs.video.picture.brightness),
        ),
        (0., 2., 0.01),
    ),
    picture_row(
        "video.picture.contrast",
        "Contrast",
        (
            Control::Slider { marks: Marks::None, format: plain },
            number!(f32: prefs.video.picture.contrast),
        ),
        (0., 2., 0.01),
    ),
    picture_row(
        "video.picture.saturation",
        "Saturation",
        (
            Control::Slider { marks: Marks::Ticks(1.), format: plain },
            number!(f32: prefs.video.picture.saturation),
        ),
        (0., 3., 0.01),
    ),
    picture_row(
        "video.picture.gamma",
        "Gamma",
        (
            Control::Slider { marks: Marks::Ticks(1.), format: plain },
            number!(f32: prefs.video.picture.gamma),
        ),
        (0.01, 10., 0.01),
    ),
    picture_row(
        "video.picture.sharpen",
        "Sharpen Sigma",
        (
            Control::Slider { marks: Marks::Ticks(0.5), format: strength },
            number!(f32: prefs.video.picture.sharpen),
        ),
        (0., 2., 0.01),
    )
    .info(
        "Sharpens edges; 0 is off. A little goes a long way. Seen at once, kept for every video.",
    ),
    picture_row(
        "video.picture.deband_radius",
        "Banding Removal Radius",
        (
            Control::Slider { marks: Marks::Ticks(4.), format: radius },
            number!(u32: prefs.video.picture.deband_radius),
        ),
        (0., 32., 4.),
    )
    .info(
        "Smooths the steps in gradients (skies, dark scenes) over this many pixels; 0 is off. \
         Seen at once, kept for every video.",
    )
    .keywords(&["deband", "gradfun", "gradient", "filter"]),
    picture_row(
        "video.picture.grain",
        "Film Grain Variance",
        (
            Control::Slider { marks: Marks::Ticks(1.), format: strength },
            number!(f32: prefs.video.picture.grain),
        ),
        (0., 10., 0.1),
    )
    .info(
        "Adds film grain, which can hide banding and compression; 0 is off. Seen at once, kept \
         for every video.",
    )
    .keywords(&["noise", "film", "texture", "filter"]),
    SettingDef::new(
        "video.reset_picture",
        PICTURE,
        "Reset picture",
        Control::Button { label: "Reset", action: reset_picture },
        Binding::ACTION,
    )
    .info(
        "Puts the picture back as the media has it: hue, brightness, contrast, saturation and \
         gamma, with sharpening, banding removal and film grain off. The player's settings \
         panel follows.",
    )
    .keywords(&["default", "color"]),
    SettingDef::new(
        "video.deinterlace",
        DEINTERLACING,
        "Deinterlacing",
        Control::Dropdown(Deinterlace::CHOICES),
        choice!(prefs.video.deinterlace),
    )
    .info(
        "Removes the comb lines of interlaced video (TV recordings, DVDs). Automatic does it \
         when the video says it is interlaced. A change reopens the video where it is.",
    )
    .keywords(&["interlaced", "comb", "tv", "dvd"]),
    SettingDef::new(
        "video.deinterlace_method",
        DEINTERLACING,
        "Method",
        Control::Dropdown(DeinterlaceMethod::CHOICES),
        choice!(prefs.video.deinterlace_method),
    )
    .info(
        "How lines are joined. Yadif is sharp, Yadif (2x) and Bob keep motion smooth at twice \
         the frame rate, Blend is cheap; Film NTSC undoes the telecine of film on NTSC discs.",
    )
    .depends_on("video.deinterlace", Condition::IsNot("off"))
    .keywords(&["yadif", "bob", "blend", "telecine"]),
];
