//! Interface: the Skin and Window sections (S5.4.1, S5.4.4). The Skin rows only exist in builds
//! with the `skins` feature, until a skinning system does (S8.14, D19).

#[cfg(feature = "skins")]
use super::super::def::{Condition, PathPick};
use super::super::def::{Control, SettingDef};
#[cfg(feature = "skins")]
use super::super::value::Value;
use super::super::value::{Choice, SettingChoice, choice, text, toggle};
use crate::app::screens;
use crate::settings::FitVideo;
use crate::settings::registry::Category;

#[cfg(feature = "skins")]
const SKIN: (Category, &str) = (Category::Interface, "Skin");
const WINDOW: (Category, &str) = (Category::Interface, "Window");

impl SettingChoice for FitVideo {
    const VALUES: &'static [Self] = &[Self::Never, Self::FirstVideo, Self::EveryVideo];
    const CHOICES: &'static [Choice] = &[
        Choice::new("never", "Never"),
        Choice::new("first_video", "For the first video"),
        Choice::new("every_video", "For every video"),
    ];
}

/// A skin file that exists; empty means none.
#[cfg(feature = "skins")]
fn skin_file(value: &Value) -> Result<(), String> {
    match value {
        Value::Text(text)
            if !text.trim().is_empty() && !std::path::Path::new(text.trim()).is_file() =>
        {
            Err("No skin file there".into())
        }
        _ => Ok(()),
    }
}

/// The Interface page's Skin and Window rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    #[cfg(feature = "skins")]
    SettingDef::new(
        "interface.use_custom_skin",
        SKIN,
        "Use a custom skin",
        Control::Toggle,
        toggle!(prefs.interface.use_custom_skin),
    )
    .info("Draws VLC's windows with the skin file below instead of the usual look.")
    .keywords(&["theme", "appearance", "look"]),
    #[cfg(feature = "skins")]
    SettingDef::new(
        "skin_file",
        SKIN,
        "Skin file",
        Control::Path(PathPick::ExistingFile),
        text!(local.skin_file),
    )
    .info("The skin VLC uses while \"Use a custom skin\" is on.")
    .validate(skin_file)
    .depends_on("interface.use_custom_skin", Condition::IsOn)
    .keywords(&["theme", "path"]),
    SettingDef::new(
        "interface.play_video_in_main_window",
        WINDOW,
        "Play video inside main window",
        Control::Toggle,
        toggle!(prefs.interface.play_video_in_main_window),
    )
    .info(
        "Shows video in the player window. Off, video plays in a window of its own while the \
         player window keeps the controls, and closing that window stops playback.",
    )
    .keywords(&["separate window", "embedded video", "video window"]),
    SettingDef::new(
        "interface.resize_to_fit_video",
        WINDOW,
        "Resize window to fit video",
        Control::Dropdown(FitVideo::CHOICES),
        choice!(prefs.interface.resize_to_fit_video),
    )
    .info(
        "Sizes the window showing video to the video's own size, as far as the screen allows. \
         A maximized or fullscreen window is never resized.",
    )
    .keywords(&["video size", "native size", "auto resize", "window size"]),
    SettingDef::new(
        "interface.remember_window_geometry",
        WINDOW,
        "Remember window size and position",
        Control::Toggle,
        toggle!(prefs.interface.remember_window_geometry),
    )
    .info("Opens VLC's windows where you left them. Off, they open centered at their usual size.")
    .keywords(&["window position", "restore", "geometry"]),
    SettingDef::new(
        "interface.keep_above_other_windows",
        WINDOW,
        "Keep window above other windows",
        Control::Toggle,
        toggle!(prefs.interface.keep_above_other_windows),
    )
    .info(
        "Keeps the player, and the video's own window, in front of other apps' windows, except \
         in fullscreen. Not available on Linux yet.",
    )
    .keywords(&["always on top", "pin", "float", "stay on top"]),
    SettingDef::new(
        "interface.show_window_decorations",
        WINDOW,
        "Show window decorations",
        Control::Toggle,
        toggle!(prefs.interface.show_window_decorations),
    )
    .info(
        "Shows the title bar and the window buttons. Off, only the video and its controls show: \
         drag the video to move the window, and right-click it for the menu.",
    )
    .keywords(&["title bar", "frame", "borderless", "chrome"]),
    SettingDef::new(
        "interface.show_file_name_in_title_bar",
        WINDOW,
        "Show the file name in the title bar",
        Control::Toggle,
        toggle!(prefs.interface.show_file_name_in_title_bar),
    )
    .info(
        "Names the media playing in the title bar and the window's title. Off, they always say \
         VLC Media Player.",
    )
    .keywords(&["window title", "caption"]),
    SettingDef::new(
        "fullscreen_display",
        WINDOW,
        "Open fullscreen on",
        Control::Options(screens::choices),
        text!(local.fullscreen_display),
    )
    .info(
        "The screen fullscreen video goes to. Current screen keeps it where the window is; if \
         the chosen screen isn't connected, so does fullscreen.",
    )
    .keywords(&["monitor", "display", "second screen", "projector"]),
];
