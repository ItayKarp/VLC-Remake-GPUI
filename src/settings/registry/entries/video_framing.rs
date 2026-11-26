//! Video's Framing (S5.8.1, S7.2): the flip and rotation, the zoom, the aspect ratio and the
//! crop every video shows with, all seen at once. The player's settings panel holds the same
//! rows (its Video tab's Defaults and Crop) and changes them for the session only (D24).
//!
//! The flip and rotation are libVLC filters, so changing them reopens the video where it is;
//! the rest the app draws itself (`video/framing.rs`).

use super::super::def::{Condition, Control, SettingDef};
use super::super::value::{Binding, Choice, SettingChoice, Value, choice, number, text, toggle};
use crate::engine::transform::{Flip, Rotation};
use crate::settings::registry::Category;
use crate::video::framing::{AspectRatio, Crop, RATIO_HELP, Zoom, parse_ratio};

const FRAMING: (Category, &str) = (Category::Video, tr_noop!("Framing"));
/// The most pixels the crop takes off one edge: the width of 8K video.
const MAX_EDGE: f64 = 7680.;
/// What the edge rows count.
const EDGE_INFO: &str = tr_noop!(
    "In the video's own pixels, not the screen's. At least 16 pixels of the picture stay however \
     much is trimmed."
);

impl SettingChoice for Flip {
    const VALUES: &'static [Self] = &[Self::None, Self::Horizontal, Self::Vertical];
    const CHOICES: &'static [Choice] = &[
        Choice::new("none", tr_noop!("None")),
        Choice::new("horizontal", tr_noop!("Horizontal")),
        Choice::new("vertical", tr_noop!("Vertical")),
    ];
}

impl SettingChoice for Rotation {
    const VALUES: &'static [Self] =
        &[Self::Upright, Self::Quarter, Self::Half, Self::ThreeQuarters];
    const CHOICES: &'static [Choice] = &[
        Choice::new("0", "0°"),
        Choice::new("90", "90°"),
        Choice::new("180", "180°"),
        Choice::new("270", "270°"),
    ];
}

impl SettingChoice for Zoom {
    const VALUES: &'static [Self] =
        &[Self::Fit, Self::Fill, Self::Original, Self::Half, Self::Double];
    const CHOICES: &'static [Choice] = &[
        Choice::new("fit", tr_noop!("Fit to Window")),
        Choice::new("fill", tr_noop!("Fill Window")),
        Choice::new("original", tr_noop!("1:1 Original")),
        Choice::new("half", tr_noop!("½ Half Size")),
        Choice::new("double", tr_noop!("2× Double Size")),
    ];
}

impl SettingChoice for AspectRatio {
    const VALUES: &'static [Self] = &[
        Self::Automatic,
        Self::Square,
        Self::Standard,
        Self::FiveByFour,
        Self::Wide,
        Self::SixteenByTen,
        Self::Ratio221,
        Self::Ultrawide,
        Self::Ratio235,
        Self::Ratio239,
        Self::Custom,
    ];
    const CHOICES: &'static [Choice] = &[
        Choice::new("automatic", tr_noop!("Default")),
        Choice::new("1:1", "1:1"),
        Choice::new("4:3", "4:3"),
        Choice::new("5:4", "5:4"),
        Choice::new("16:9", "16:9"),
        Choice::new("16:10", "16:10"),
        Choice::new("2.21:1", "2.21:1"),
        Choice::new("21:9", "21:9"),
        Choice::new("2.35:1", "2.35:1"),
        Choice::new("2.39:1", "2.39:1"),
        Choice::new("custom", tr_noop!("Custom")),
    ];
}

impl SettingChoice for Crop {
    const VALUES: &'static [Self] = &[
        Self::None,
        Self::Square,
        Self::Standard,
        Self::Wide,
        Self::SixteenByTen,
        Self::Ratio185,
        Self::Ratio221,
        Self::Ratio235,
        Self::Ratio239,
    ];
    const CHOICES: &'static [Choice] = &[
        Choice::new("none", tr_noop!("Default")),
        Choice::new("1:1", "1:1"),
        Choice::new("4:3", "4:3"),
        Choice::new("16:9", "16:9"),
        Choice::new("16:10", "16:10"),
        Choice::new("1.85:1", "1.85:1"),
        Choice::new("2.21:1", "2.21:1"),
        Choice::new("2.35:1", "2.35:1"),
        Choice::new("2.39:1", "2.39:1"),
    ];
}

/// Says when the player's panel changed the framing for this session, so this page isn't what
/// shows (D24).
fn framing_overridden(cx: &gpui::App) -> Option<String> {
    crate::effects::framing_changed(cx).then(|| {
        tr!(
            "The player's settings panel changed the framing for this session; its Save as Default \
             writes it here."
        )
        .to_owned()
    })
}

/// Checks a typed custom ratio.
fn custom_ratio(value: &Value) -> Result<(), String> {
    let Value::Text(text) = value else { return Ok(()) };
    parse_ratio(text).map(|_| ()).ok_or_else(|| crate::i18n::text(RATIO_HELP).to_owned())
}

/// A Trim edges row: the pixels off one edge.
const fn edge(key: &'static str, label: &'static str, binding: Binding) -> SettingDef {
    SettingDef::new(key, FRAMING, label, Control::Number { unit: tr_noop!("px") }, binding)
        .info(EDGE_INFO)
        .range(0., MAX_EDGE, 1.)
        .depends_on("video.crop_edges.enabled", Condition::IsOn)
        .keywords(&["crop", "trim", "edge", "black bars", "pixels"])
}

/// The Framing rows.
pub(in crate::settings::registry) static ENTRIES: &[SettingDef] = &[
    SettingDef::new(
        "video.flip",
        FRAMING,
        tr_noop!("Flip"),
        Control::Segmented(Flip::CHOICES),
        choice!(prefs.video.flip),
    )
    .info(tr_noop!("Mirrors the picture. A change reopens the video where it is."))
    .hint(framing_overridden)
    .keywords(&["mirror", "flip", "reverse"]),
    SettingDef::new(
        "video.rotation",
        FRAMING,
        tr_noop!("Rotate"),
        Control::Dropdown(Rotation::CHOICES),
        choice!(prefs.video.rotation),
    )
    .info(tr_noop!(
        "Turns the picture clockwise, after any flip, on top of the turn the file records. A \
         change reopens the video where it is."
    ))
    .keywords(&["rotate", "turn", "portrait", "sideways", "orientation"]),
    SettingDef::new(
        "video.zoom",
        FRAMING,
        tr_noop!("Zoom"),
        Control::Dropdown(Zoom::CHOICES),
        choice!(prefs.video.zoom),
    )
    .info(tr_noop!(
        "How large the picture is drawn. Fill crops what doesn't fit; 1:1 shows each video pixel \
         as one screen pixel, centered, cut off where larger than the window."
    ))
    .keywords(&["zoom", "scale", "size", "fit", "fill", "original"]),
    SettingDef::new(
        "video.aspect_ratio",
        FRAMING,
        tr_noop!("Aspect ratio"),
        Control::Dropdown(AspectRatio::CHOICES),
        choice!(prefs.video.aspect_ratio),
    )
    .info(tr_noop!("The shape the picture is stretched to. Default keeps the media's own."))
    .keywords(&["stretch", "widescreen", "16:9", "4:3", "21:9"]),
    SettingDef::new(
        "video.custom_aspect_ratio",
        FRAMING,
        tr_noop!("Custom ratio"),
        Control::Text,
        text!(prefs.video.custom_aspect_ratio),
    )
    .info(tr_noop!(
        "The shape Custom stretches the picture to, as width:height: 21:9, 2.35:1 or 2.4."
    ))
    .placeholder("2.4:1")
    .validate(custom_ratio)
    .depends_on("video.aspect_ratio", Condition::Is("custom"))
    .keywords(&["stretch", "custom", "ratio", "shape"]),
    SettingDef::new(
        "video.crop",
        FRAMING,
        tr_noop!("Crop"),
        Control::Dropdown(Crop::CHOICES),
        choice!(prefs.video.crop),
    )
    .info(tr_noop!("Trims the picture to this shape, keeping its middle, to hide black bars."))
    .keywords(&["black bars", "trim", "zoom", "letterbox"]),
    SettingDef::new(
        "video.crop_edges.enabled",
        FRAMING,
        tr_noop!("Trim edges"),
        Control::Toggle,
        toggle!(prefs.video.crop_edges.enabled),
    )
    .info(tr_noop!(
        "Trims pixels off each edge, before the crop's shape. The player's Crop panel sets them."
    ))
    .keywords(&["crop", "trim", "edge", "black bars", "pixels"]),
    edge("video.crop_edges.top", tr_noop!("Top"), number!(u32: prefs.video.crop_edges.top)),
    edge(
        "video.crop_edges.bottom",
        tr_noop!("Bottom"),
        number!(u32: prefs.video.crop_edges.bottom),
    ),
    edge("video.crop_edges.left", tr_noop!("Left"), number!(u32: prefs.video.crop_edges.left)),
    edge("video.crop_edges.right", tr_noop!("Right"), number!(u32: prefs.video.crop_edges.right)),
];
